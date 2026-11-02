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

use asyncband::once::OnceCell;
use sqlx::postgres::PgConnectOptions;

use super::POSTGRESQL_SCHEME;
use super::config::PostgresqlConfig;
use super::core::*;
use super::deleter::PostgresqlDeleter;
use super::reader::*;
use super::writer::PostgresqlWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// [PostgreSQL](https://www.postgresql.org/) services support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct PostgresqlBuilder {
    pub(super) config: PostgresqlConfig,
}

impl PostgresqlBuilder {
    /// Set the connection url string of the postgresql service.
    ///
    /// The URL should be with a scheme of either `postgres://` or `postgresql://`.
    ///
    /// - `postgresql://user@localhost`
    /// - `postgresql://user:password@%2Fvar%2Flib%2Fpostgresql/mydb?connect_timeout=10`
    /// - `postgresql://user@host1:1234,host2,host3:5678?target_session_attrs=read-write`
    /// - `postgresql:///mydb?user=user&host=/var/lib/postgresql`
    ///
    /// For more information, please visit <https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgConnectOptions.html>.
    pub fn connection_string(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.connection_string = Some(v.to_string());
        }
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

    /// Set the table name of the postgresql service to read/write.
    pub fn table(mut self, table: &str) -> Self {
        if !table.is_empty() {
            self.config.table = Some(table.to_string());
        }
        self
    }

    /// Set the key field name of the postgresql service to read/write.
    ///
    /// Default to `key` if not specified.
    pub fn key_field(mut self, key_field: &str) -> Self {
        if !key_field.is_empty() {
            self.config.key_field = Some(key_field.to_string());
        }
        self
    }

    /// Set the value field name of the postgresql service to read/write.
    ///
    /// Default to `value` if not specified.
    pub fn value_field(mut self, value_field: &str) -> Self {
        if !value_field.is_empty() {
            self.config.value_field = Some(value_field.to_string());
        }
        self
    }
}

impl Builder for PostgresqlBuilder {
    type Config = PostgresqlConfig;

    fn build(self) -> Result<impl Service> {
        let conn = match self.config.connection_string {
            Some(v) => v,
            None => {
                return Err(
                    Error::new(ErrorKind::ConfigInvalid, "connection_string is empty")
                        .with_context("service", POSTGRESQL_SCHEME),
                );
            }
        };

        let config = conn.parse::<PgConnectOptions>().map_err(|err| {
            Error::new(ErrorKind::ConfigInvalid, "connection_string is invalid")
                .with_context("service", POSTGRESQL_SCHEME)
                .set_source(err)
        })?;

        let table = match self.config.table {
            Some(v) => v,
            None => {
                return Err(Error::new(ErrorKind::ConfigInvalid, "table is empty")
                    .with_context("service", POSTGRESQL_SCHEME));
            }
        };

        let key_field = self.config.key_field.unwrap_or_else(|| "key".to_string());

        let value_field = self
            .config
            .value_field
            .unwrap_or_else(|| "value".to_string());

        let root = normalize_root(self.config.root.unwrap_or_else(|| "/".to_string()).as_str());

        Ok(PostgresqlBackend::new(PostgresqlCore {
            pool: OnceCell::new(),
            config,
            table,
            key_field,
            value_field,
        })
        .with_normalized_root(root))
    }
}

/// Backend for Postgresql service
#[derive(Clone, Debug)]
pub struct PostgresqlBackend {
    pub(crate) core: Arc<PostgresqlCore>,
    pub(crate) root: String,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl PostgresqlBackend {
    pub fn new(core: PostgresqlCore) -> Self {
        let info = ServiceInfo::new(POSTGRESQL_SCHEME, "/", &core.table);
        let capability = Capability {
            read: true,
            stat: true,
            write: true,
            write_can_empty: true,
            delete: true,
            shared: true,
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

impl Service for PostgresqlBackend {
    type Reader = oio::StreamReader<PostgresqlReader>;
    type Writer = PostgresqlWriter;
    type Lister = ();
    type Deleter = oio::OneShotDeleter<PostgresqlDeleter>;
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
            match self.core.get_length(&p).await? {
                Some(length) => Ok(RpStat::new({
                    let metadata = MetadataBuilder::file(length as u64);
                    metadata.build()
                })),
                None => Err(Error::new(
                    ErrorKind::NotFound,
                    "kv not found in postgresql",
                )),
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<PostgresqlReader> = {
            Ok(oio::StreamReader::new(PostgresqlReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: PostgresqlWriter = {
            let p = build_abs_path(&self.root, path);
            Ok(PostgresqlWriter::new(self.core.clone(), p))
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<PostgresqlDeleter> = {
            Ok(oio::OneShotDeleter::new(PostgresqlDeleter::new(
                self.core.clone(),
                self.root.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, _path: &str, _args: OpList) -> Result<Self::Lister> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
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
