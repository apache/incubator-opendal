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

use std::fmt::Debug;
use std::sync::Arc;

use super::REDB_SCHEME;
use super::config::RedbConfig;
use super::core::*;
use super::deleter::RedbDeleter;
use super::lister::RedbLister;
use super::reader::*;
use super::writer::RedbWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// Redb service support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct RedbBuilder {
    pub(super) config: RedbConfig,

    pub(super) database: Option<Arc<redb::Database>>,
}

impl Debug for RedbBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedbBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl RedbBuilder {
    /// Set the database for Redb.
    ///
    /// This method should be called when you want to
    /// use multiple tables of one database because
    /// Redb doesn't allow opening a database that have been opened.
    ///
    /// <div class="warning">
    ///
    /// `datadir` and `database` should not be set simultaneously.
    /// If both are set, `database` will take precedence.
    ///
    /// </div>
    pub fn database(mut self, db: Arc<redb::Database>) -> Self {
        self.database = Some(db);
        self
    }

    /// Set the path to the redb data directory. Will create if not exists.
    ///
    ///
    /// <div class="warning">
    ///
    /// Opening redb database via `datadir` takes away the ability to access multiple redb tables.
    /// If you need to access multiple redb tables, the correct solution is to
    /// create an `Arc<redb::database>` beforehand and then share it via [`database`]
    /// with multiple builders where every builder will open one redb table.
    ///
    /// </div>
    ///
    /// [`database`]: RedbBuilder::database
    pub fn datadir(mut self, path: &str) -> Self {
        self.config.datadir = Some(path.into());
        self
    }

    /// Set the table name for Redb. Will create if not exists.
    pub fn table(mut self, table: &str) -> Self {
        self.config.table = Some(table.into());
        self
    }

    /// Set the root for Redb.
    pub fn root(mut self, path: &str) -> Self {
        self.config.root = Some(path.into());
        self
    }
}

impl Builder for RedbBuilder {
    type Config = RedbConfig;

    fn build(self) -> Result<impl Service> {
        let table_name = self.config.table.ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "table is required but not set")
                .with_context("service", REDB_SCHEME)
        })?;

        let (datadir, db) = if let Some(db) = self.database {
            (None, db)
        } else {
            let datadir = self.config.datadir.ok_or_else(|| {
                Error::new(ErrorKind::ConfigInvalid, "datadir is required but not set")
                    .with_context("service", REDB_SCHEME)
            })?;

            let db = redb::Database::create(&datadir)
                .map_err(parse_database_error)?
                .into();

            (Some(datadir), db)
        };

        create_table(&db, &table_name)?;

        let root = normalize_root(&self.config.root.unwrap_or_default());

        Ok(RedbBackend::new(RedbCore {
            datadir,
            table: table_name,
            db,
        })
        .with_normalized_root(root))
    }
}

/// Backend for Redb services.
#[derive(Clone, Debug)]
pub struct RedbBackend {
    pub(crate) core: Arc<RedbCore>,
    pub(crate) root: String,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl RedbBackend {
    pub fn new(core: RedbCore) -> Self {
        let info = ServiceInfo::new(REDB_SCHEME, "/", &core.table);
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

impl Service for RedbBackend {
    type Reader = oio::StreamReader<RedbReader>;
    type Writer = RedbWriter;
    type Lister = oio::HierarchyLister<RedbLister>;
    type Deleter = oio::OneShotDeleter<RedbDeleter>;
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
                None => Err(Error::new(ErrorKind::NotFound, "kv not found in redb")),
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<RedbReader> = {
            Ok(oio::StreamReader::new(RedbReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: RedbWriter = {
            let p = build_abs_path(&self.root, path);
            Ok(RedbWriter::new(self.core.clone(), p))
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<RedbDeleter> = {
            Ok(oio::OneShotDeleter::new(RedbDeleter::new(
                self.core.clone(),
                self.root.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let prefix = build_abs_path(&self.root, path);
        let range = self.core.scan(&prefix)?;
        let lister = RedbLister::new(self.root.clone(), prefix, range);
        Ok(oio::HierarchyLister::new(lister, path, args.recursive()))
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
