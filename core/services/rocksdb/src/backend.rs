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

use std::sync::Arc;

use opendal_core::raw::*;
use opendal_core::*;
use rocksdb::DB;

use super::ROCKSDB_SCHEME;
use super::config::RocksdbConfig;
use super::core::*;
use super::deleter::RocksdbDeleter;
use super::lister::RocksdbLister;
use super::reader::*;
use super::writer::RocksdbWriter;

/// RocksDB service support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct RocksdbBuilder {
    pub(super) config: RocksdbConfig,
}

impl RocksdbBuilder {
    /// Set the path to the rocksdb data directory. Creates if not exists.
    pub fn datadir(mut self, path: &str) -> Self {
        self.config.datadir = Some(path.into());
        self
    }

    /// Set the working directory, all operations will be performed under it.
    ///
    /// default: "/"
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }
}

impl Builder for RocksdbBuilder {
    type Config = RocksdbConfig;

    fn build(self) -> Result<impl Service> {
        let path = self.config.datadir.ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "datadir is required but not set")
                .with_context("service", ROCKSDB_SCHEME)
        })?;
        let db = DB::open_default(&path).map_err(|e| {
            Error::new(ErrorKind::ConfigInvalid, "open default transaction db")
                .with_context("service", ROCKSDB_SCHEME)
                .with_context("datadir", path)
                .set_source(e)
        })?;

        let root = normalize_root(&self.config.root.unwrap_or_default());

        Ok(RocksdbBackend::new(RocksdbCore { db: Arc::new(db) }).with_normalized_root(root))
    }
}

/// Backend for rocksdb service.
#[derive(Clone, Debug)]
pub struct RocksdbBackend {
    pub(crate) core: Arc<RocksdbCore>,
    pub(crate) root: String,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl RocksdbBackend {
    pub fn new(core: RocksdbCore) -> Self {
        let info = ServiceInfo::new(ROCKSDB_SCHEME, "/", core.db.path().to_string_lossy());
        let capability = Capability {
            read: true,
            stat: true,
            write: true,
            write_can_empty: true,
            delete: true,
            list: true,
            list_with_recursive: true,
            ..Default::default()
        };

        Self {
            core: Arc::new(core),
            root: "/".to_string(),
            info,
            capability,
        }
    }

    fn with_normalized_root(mut self, root: String) -> Self {
        self.info = self.info.with_root(&root);
        self.root = root;
        self
    }
}

impl Service for RocksdbBackend {
    type Reader = oio::StreamReader<RocksdbReader>;
    type Writer = RocksdbWriter;
    type Lister = oio::HierarchyLister<RocksdbLister>;
    type Deleter = oio::OneShotDeleter<RocksdbDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.info.clone()
    }

    fn capability(&self) -> Capability {
        self.capability
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let p = build_abs_path(&self.root, path);

        if p == build_abs_path(&self.root, "") {
            Ok(RpStat::new(MetadataBuilder::dir().build()))
        } else {
            let bs = self.core.get(&p)?;
            match bs {
                Some(bs) => Ok(RpStat::new({
                    let metadata = MetadataBuilder::file(bs.len() as u64);
                    metadata.build()
                })),
                None => Err(Error::new(ErrorKind::NotFound, "kv not found in rocksdb")),
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<RocksdbReader> = {
            Ok(oio::StreamReader::new(RocksdbReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: RocksdbWriter = {
            let p = build_abs_path(&self.root, path);
            let writer = RocksdbWriter::new(self.core.clone(), p);
            Ok(writer)
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<RocksdbDeleter> = {
            let deleter = RocksdbDeleter::new(self.core.clone(), self.root.clone());
            Ok(oio::OneShotDeleter::new(deleter))
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::HierarchyLister<RocksdbLister> = {
            let p = build_abs_path(&self.root, path);
            let lister = RocksdbLister::new(self.core.clone(), self.root.clone(), p)?;
            Ok(oio::HierarchyLister::new(lister, path, args.recursive()))
        }?;

        Ok(output)
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
        _from: &str,
        _to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
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
