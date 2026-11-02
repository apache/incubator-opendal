// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use std::io;
use std::sync::Arc;

use log::debug;

use super::HDFS_SCHEME;
use super::config::HdfsConfig;
use super::core::HdfsCore;
use super::deleter::HdfsDeleter;
use super::lister::HdfsLister;
use super::reader::*;
use opendal_core::raw::*;
use opendal_core::*;

#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct HdfsBuilder {
    pub(super) config: HdfsConfig,
}

impl HdfsBuilder {
    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set name_node of this backend.
    ///
    /// Valid format including:
    ///
    /// - `default`: using the default setting based on hadoop config.
    /// - `hdfs://127.0.0.1:9000`: connect to hdfs cluster.
    pub fn name_node(mut self, name_node: &str) -> Self {
        if !name_node.is_empty() {
            self.config.name_node = Some(name_node.to_string())
        }

        self
    }

    /// Set kerberos_ticket_cache_path of this backend
    ///
    /// This should be configured when kerberos is enabled.
    pub fn kerberos_ticket_cache_path(mut self, kerberos_ticket_cache_path: &str) -> Self {
        if !kerberos_ticket_cache_path.is_empty() {
            self.config.kerberos_ticket_cache_path = Some(kerberos_ticket_cache_path.to_string())
        }
        self
    }

    /// Set user of this backend
    pub fn user(mut self, user: &str) -> Self {
        if !user.is_empty() {
            self.config.user = Some(user.to_string())
        }
        self
    }

    /// Deprecated: HDFS append capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "HDFS append capability is enabled by default and this option is no longer needed."
    )]
    pub fn enable_append(self, _enable_append: bool) -> Self {
        self
    }

    /// Set temp dir for atomic write.
    ///
    /// # Notes
    ///
    /// - When append is enabled, we will not use atomic write
    ///   to avoid data loss and performance issue.
    pub fn atomic_write_dir(mut self, dir: &str) -> Self {
        self.config.atomic_write_dir = if dir.is_empty() {
            None
        } else {
            Some(String::from(dir))
        };
        self
    }
}

impl Builder for HdfsBuilder {
    type Config = HdfsConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let name_node = match &self.config.name_node {
            Some(v) => v,
            None => {
                return Err(Error::new(ErrorKind::ConfigInvalid, "name node is empty")
                    .with_context("service", HDFS_SCHEME));
            }
        };

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let mut builder = hdrs::ClientBuilder::new(name_node);
        if let Some(ticket_cache_path) = &self.config.kerberos_ticket_cache_path {
            builder = builder.with_kerberos_ticket_cache_path(ticket_cache_path.as_str());
        }
        if let Some(user) = &self.config.user {
            builder = builder.with_user(user.as_str());
        }

        let client = builder.connect().map_err(new_std_io_error)?;

        // Create root dir if not exist.
        if let Err(e) = client.metadata(&root)
            && e.kind() == io::ErrorKind::NotFound
        {
            debug!("root {root} is not exist, creating now");

            client.create_dir(&root).map_err(new_std_io_error)?
        }

        let atomic_write_dir = self.config.atomic_write_dir;

        // If atomic write dir is not exist, we must create it.
        if let Some(d) = &atomic_write_dir
            && let Err(e) = client.metadata(d)
            && e.kind() == io::ErrorKind::NotFound
        {
            client.create_dir(d).map_err(new_std_io_error)?
        }

        Ok(HdfsBackend {
            core: Arc::new(HdfsCore {
                info: ServiceInfo::new(HDFS_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,

                    read: true,

                    write: true,
                    write_can_append: true,

                    create_dir: true,
                    delete: true,
                    delete_with_recursive: true,

                    list: true,

                    copy: true,

                    rename: true,
                    rename_with_if_not_exists: true,

                    shared: true,

                    ..Default::default()
                },
                root,
                atomic_write_dir,
                client: Arc::new(client),
            }),
        })
    }
}

/// Backend for hdfs services.
#[derive(Debug, Clone)]
pub struct HdfsBackend {
    pub(crate) core: Arc<HdfsCore>,
}

impl Service for HdfsBackend {
    type Reader = oio::PositionReader<HdfsReader>;
    type Writer = HdfsLazyWriter;
    type Lister = Option<HdfsLister>;
    type Deleter = oio::OneShotDeleter<HdfsDeleter>;
    type Copier = oio::OneShotCopier;
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info.clone()
    }

    fn capability(&self) -> Capability {
        self.core.capability
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.core.hdfs_create_dir(path)?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let m = self.core.hdfs_stat(path)?;
        Ok(RpStat::new(m))
    }
    fn read(&self, _ctx: &OperationContext, path: &str, _: OpRead) -> Result<Self::Reader> {
        Ok(oio::PositionReader::new(HdfsReader::new(
            self.core.clone(),
            path,
        )))
    }

    fn write(&self, _ctx: &OperationContext, path: &str, op: OpWrite) -> Result<Self::Writer> {
        Ok(HdfsLazyWriter::new(self.core.clone(), path, op))
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<HdfsDeleter> = {
            Ok(oio::OneShotDeleter::new(HdfsDeleter::new(Arc::clone(
                &self.core,
            ))))
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, _: OpList) -> Result<Self::Lister> {
        let output: Option<HdfsLister> = {
            match self.core.hdfs_list(path)? {
                Some(f) => {
                    let rd = HdfsLister::new(&self.core.root, f, path);
                    Ok(Some(rd))
                }
                None => Ok(None),
            }
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        _ctx: &OperationContext,
        from: &str,
        to: &str,
        _args: OpCopy,
    ) -> Result<Self::Copier> {
        let core = self.core.clone();
        let from = from.to_string();
        let to = to.to_string();
        // Recreate the future so retry layers can rerun the copy after a temporary error.
        Ok(oio::OneShotCopier::new_with(move || {
            let core = core.clone();
            let from = from.clone();
            let to = to.clone();
            async move { core.hdfs_copy(&from, &to).await }
        }))
    }

    async fn rename(
        &self,
        _ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.core.hdfs_rename(from, to, &args)?;
        Ok(RpRename::new())
    }

    async fn presign(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpPresign,
    ) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}
