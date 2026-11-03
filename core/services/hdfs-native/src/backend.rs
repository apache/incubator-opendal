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

use std::collections::HashMap;
use std::sync::Arc;

use log::debug;

use super::HDFS_NATIVE_SCHEME;
use super::config::HDFS_DEFAULT_AUTHORITY;
use super::config::HDFS_SCHEME_PREFIX;
use super::config::HdfsNativeConfig;
use super::config::init_hdfs_config;
use super::core::HdfsNativeCore;
use super::core::parse_hdfs_error;
use super::deleter::HdfsNativeDeleter;
use super::reader::*;
use opendal_core::raw::*;
use opendal_core::*;

/// [Hadoop Distributed File System (HDFS™)](https://hadoop.apache.org/) support.
/// Using [Native Rust HDFS client](https://github.com/Kimahriman/hdfs-native).
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct HdfsNativeBuilder {
    pub(super) config: HdfsNativeConfig,
}

impl HdfsNativeBuilder {
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
    /// - `hdfs://namenode1:9000,namenode2:9000`: connect to hdfs ha cluster.
    pub fn name_node(mut self, name_node: &str) -> Self {
        if !name_node.is_empty() {
            // Trim trailing `/` so that we can accept `hdfs://127.0.0.1:9000/`
            self.config.name_node = Some(name_node.trim_end_matches('/').to_string())
        }

        self
    }

    /// Deprecated: HDFS Native append capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "HDFS Native append capability is enabled by default and this option is no longer needed."
    )]
    pub fn enable_append(self, _enable_append: bool) -> Self {
        self
    }

    /// Set other hdfs-native client options of this backend.
    ///
    /// See the [hdfs-native documentation](https://github.com/Kimahriman/hdfs-native)
    /// for the supported options.
    pub fn options(mut self, options: HashMap<String, String>) -> Self {
        self.config.options = Some(options);
        self
    }
}

impl Builder for HdfsNativeBuilder {
    type Config = HdfsNativeConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let name_node = match &self.config.name_node {
            Some(v) => v,
            None => {
                return Err(Error::new(ErrorKind::ConfigInvalid, "name_node is empty")
                    .with_context("service", HDFS_NATIVE_SCHEME));
            }
        };

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let mut hdfs_config = init_hdfs_config(name_node);
        if let Some(options) = &self.config.options {
            hdfs_config.extend(options.iter().map(|(k, v)| (k.clone(), v.clone())));
        }

        let client = hdfs_native::ClientBuilder::new()
            .with_url(format!("{}{}", HDFS_SCHEME_PREFIX, HDFS_DEFAULT_AUTHORITY))
            .with_config(hdfs_config)
            .build()
            .map_err(parse_hdfs_error)?;

        // need to check if root dir exists, create if not
        Ok(HdfsNativeBackend {
            core: Arc::new(HdfsNativeCore {
                info: ServiceInfo::new(HDFS_NATIVE_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,

                    read: true,

                    write: true,
                    write_can_append: true,

                    create_dir: true,
                    delete: true,

                    list: true,

                    rename: true,
                    rename_with_if_not_exists: true,

                    shared: true,

                    ..Default::default()
                },
                root,
                client: Arc::new(client),
            }),
        })
    }
}

// #[inline]
// fn tmp_file_of(path: &str) -> String {
//     let name = get_basename(path);
//     let uuid = Uuid::new_v4().to_string();

//     format!("{name}.{uuid}")
// }

/// Backend for hdfs-native services.
#[derive(Debug, Clone)]
pub struct HdfsNativeBackend {
    pub(crate) core: Arc<HdfsNativeCore>,
}

impl Service for HdfsNativeBackend {
    type Reader = oio::PositionReader<HdfsNativeReader>;
    type Writer = HdfsNativeLazyWriter;
    type Lister = HdfsNativeLazyLister;
    type Deleter = oio::OneShotDeleter<HdfsNativeDeleter>;
    type Copier = ();
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
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.core.hdfs_create_dir(path).await?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let m = self.core.hdfs_stat(path).await?;
        Ok(RpStat::new(m))
    }
    fn read(&self, _ctx: &OperationContext, path: &str, _: OpRead) -> Result<Self::Reader> {
        Ok(oio::PositionReader::new(HdfsNativeReader::new(
            self.core.clone(),
            path,
        )))
    }

    fn write(&self, _ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        Ok(HdfsNativeLazyWriter::new(self.core.clone(), path, args))
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<HdfsNativeDeleter> = {
            Ok(oio::OneShotDeleter::new(HdfsNativeDeleter::new(
                Arc::clone(&self.core),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        Ok(HdfsNativeLazyLister::new(self.core.clone(), path))
    }

    fn copy(
        &self,
        _ctx: &OperationContext,
        _from: &str,
        _to: &str,
        _args: OpCopy,
    ) -> Result<Self::Copier> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn rename(
        &self,
        _ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.core.hdfs_rename(from, to, &args).await?;
        Ok(RpRename::default())
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
