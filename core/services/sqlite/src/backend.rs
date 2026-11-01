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

use std::str::FromStr;
use std::sync::Arc;

use asyncband::once::OnceCell;
use sqlx::sqlite::SqliteConnectOptions;

use super::SQLITE_SCHEME;
use super::config::SqliteConfig;
use super::core::SqliteCore;
use super::deleter::SqliteDeleter;
use super::reader::*;
use super::writer::SqliteWriter;
use opendal_core::raw::oio;
use opendal_core::raw::*;
use opendal_core::*;

#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct SqliteBuilder {
    pub(super) config: SqliteConfig,
}

impl SqliteBuilder {
    /// Set the connection_string of the sqlite service.
    ///
    /// This connection string is used to connect to the sqlite service. There are url based formats:
    ///
    /// ## Url
    ///
    /// This format resembles the url format of the sqlite client:
    ///
    /// - `sqlite::memory:`
    /// - `sqlite:data.db`
    /// - `sqlite://data.db`
    ///
    /// For more information, please visit <https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html>.
    pub fn connection_string(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.connection_string = Some(v.to_string());
        }
        self
    }

    /// set the working directory, all operations will be performed under it.
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

    /// Set the table name of the sqlite service to read/write.
    pub fn table(mut self, table: &str) -> Self {
        if !table.is_empty() {
            self.config.table = Some(table.to_string());
        }
        self
    }

    /// Set the key field name of the sqlite service to read/write.
    ///
    /// Default to `key` if not specified.
    pub fn key_field(mut self, key_field: &str) -> Self {
        if !key_field.is_empty() {
            self.config.key_field = Some(key_field.to_string());
        }
        self
    }

    /// Set the value field name of the sqlite service to read/write.
    ///
    /// Default to `value` if not specified.
    pub fn value_field(mut self, value_field: &str) -> Self {
        if !value_field.is_empty() {
            self.config.value_field = Some(value_field.to_string());
        }
        self
    }
}

impl Builder for SqliteBuilder {
    type Config = SqliteConfig;

    fn build(self) -> Result<impl Service> {
        let conn = match self.config.connection_string {
            Some(v) => v,
            None => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "connection_string is required but not set",
                )
                .with_context("service", SQLITE_SCHEME));
            }
        };

        let config = SqliteConnectOptions::from_str(&conn).map_err(|err| {
            Error::new(ErrorKind::ConfigInvalid, "connection_string is invalid")
                .with_context("service", SQLITE_SCHEME)
                .set_source(err)
        })?;

        let table = match self.config.table {
            Some(v) => v,
            None => {
                return Err(Error::new(ErrorKind::ConfigInvalid, "table is empty")
                    .with_context("service", SQLITE_SCHEME));
            }
        };

        let key_field = self.config.key_field.unwrap_or_else(|| "key".to_string());

        let value_field = self
            .config
            .value_field
            .unwrap_or_else(|| "value".to_string());

        let root = normalize_root(self.config.root.as_deref().unwrap_or("/"));

        Ok(SqliteBackend::new(SqliteCore {
            pool: OnceCell::new(),
            config,
            table,
            key_field,
            value_field,
        })
        .with_normalized_root(root))
    }
}

pub fn parse_sqlite_error(err: sqlx::Error) -> Error {
    let is_temporary = matches!(
        &err,
        sqlx::Error::Database(db_err) if db_err.code().is_some_and(|c| c == "5" || c == "6")
    );

    let message = if is_temporary {
        "database is locked or busy"
    } else {
        "unhandled error from sqlite"
    };

    let mut error = Error::new(ErrorKind::Unexpected, message).set_source(err);
    if is_temporary {
        error = error.set_temporary();
    }
    error
}

/// SqliteBackend implements [`Service`] for SQLite-backed object storage.
#[derive(Debug, Clone)]
pub struct SqliteBackend {
    pub(crate) core: Arc<SqliteCore>,
    pub(crate) root: String,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl SqliteBackend {
    fn new(core: SqliteCore) -> Self {
        let info = ServiceInfo::new(SQLITE_SCHEME, "/", &core.table);
        let capability = Capability {
            read: true,
            write: true,
            create_dir: true,
            delete: true,
            stat: true,
            write_can_empty: true,
            list: false,
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

impl Service for SqliteBackend {
    type Reader = oio::StreamReader<SqliteReader>;
    type Writer = SqliteWriter;
    type Lister = ();
    type Deleter = oio::OneShotDeleter<SqliteDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.info.clone()
    }

    fn capability(&self) -> Capability {
        self.capability
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let p = build_abs_path(&self.root, path);

        if p == build_abs_path(&self.root, "") {
            Ok(RpStat::new(MetadataBuilder::dir().build()))
        } else {
            let length = self.core.get_length(&p).await?;
            match length {
                Some(length) => {
                    let metadata = if p.ends_with('/') {
                        MetadataBuilder::dir()
                    } else {
                        MetadataBuilder::file(length as u64)
                    };
                    Ok(RpStat::new(metadata.build()))
                }
                None => {
                    // Check if this might be a directory by looking for keys with this prefix
                    let dir_path = if p.ends_with('/') {
                        p.clone()
                    } else {
                        format!("{}/", p)
                    };
                    let count = self.core.count_under(&dir_path).await?;

                    if count > 0 {
                        // Directory exists (has children)
                        Ok(RpStat::new(MetadataBuilder::dir().build()))
                    } else {
                        Err(Error::new(ErrorKind::NotFound, "key not found in sqlite"))
                    }
                }
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<SqliteReader> = {
            Ok(oio::StreamReader::new(SqliteReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: SqliteWriter = {
            let p = build_abs_path(&self.root, path);
            Ok(SqliteWriter::new(self.core.clone(), &p))
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<SqliteDeleter> = {
            Ok(oio::OneShotDeleter::new(SqliteDeleter::new(
                self.core.clone(),
                self.root.clone(),
            )))
        }?;

        Ok(output)
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let p = build_abs_path(&self.root, path);

        // Ensure path ends with '/' for directory marker
        let dir_path = if p.ends_with('/') {
            p
        } else {
            format!("{}/", p)
        };

        // Store directory marker with empty content
        self.core.set(&dir_path, Buffer::new()).await?;

        Ok(RpCreateDir::default())
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

#[cfg(test)]
mod test {
    use super::*;
    use opendal_core::raw::oio::Read as _;
    use opendal_core::raw::oio::ReadStream as _;
    use opendal_core::raw::oio::Write as _;
    use sqlx::SqlitePool;

    async fn build_client() -> OnceCell<SqlitePool> {
        let config = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let pool = SqlitePool::connect_with(config).await.unwrap();
        OnceCell::from_value(pool)
    }

    async fn build_backend() -> SqliteBackend {
        let core = SqliteCore {
            pool: build_client().await,
            config: Default::default(),
            table: "test_table".to_string(),
            key_field: "key".to_string(),
            value_field: "value".to_string(),
        };

        SqliteBackend::new(core)
    }

    #[tokio::test]
    async fn test_sqlite_backend_creation() {
        let backend = build_backend().await;

        // Verify basic properties
        assert_eq!(backend.root, "/");
        assert_eq!(backend.info.scheme(), SQLITE_SCHEME);
        assert!(backend.capability().read);
        assert!(backend.capability().write);
        assert!(backend.capability().delete);
        assert!(backend.capability().stat);
    }

    #[tokio::test]
    async fn test_sqlite_backend_with_root() {
        let backend = build_backend()
            .await
            .with_normalized_root("/test/".to_string());

        assert_eq!(backend.root, "/test/");
        assert_eq!(backend.info.root(), Arc::from("/test/"));
    }

    #[tokio::test]
    async fn test_sqlite_read_range_from_offset_reads_to_eof() {
        let backend = build_backend().await;

        let pool = backend.core.get_client().await.unwrap();
        sqlx::query("CREATE TABLE test_table (key TEXT PRIMARY KEY, value BLOB)")
            .execute(pool)
            .await
            .unwrap();

        let ctx = OperationContext::new();
        let mut writer = backend.write(&ctx, "hello", OpWrite::default()).unwrap();
        writer.write(Buffer::from("hello world")).await.unwrap();
        writer.close().await.unwrap();

        let reader = backend.read(&ctx, "hello", OpRead::default()).unwrap();
        let (_, mut stream) = reader.open(BytesRange::from(6_u64..)).await.unwrap();
        let buffer = stream.read_all().await.unwrap();

        assert_eq!(buffer.to_vec(), b"world");
    }

    #[tokio::test]
    async fn test_sqlite_stat_uses_value_length() {
        let backend = build_backend().await;

        let pool = backend.core.get_client().await.unwrap();
        sqlx::query("CREATE TABLE test_table (key TEXT PRIMARY KEY, value BLOB)")
            .execute(pool)
            .await
            .unwrap();

        let ctx = OperationContext::new();
        let mut writer = backend.write(&ctx, "key_id", OpWrite::default()).unwrap();
        writer.write(Buffer::from("hello world")).await.unwrap();
        writer.close().await.unwrap();

        let rp = backend
            .stat(&ctx, "key_id", OpStat::default())
            .await
            .unwrap();

        assert_eq!(rp.into_metadata().content_length(), 11);
    }

    #[tokio::test]
    async fn test_sqlite_stat_returns_byte_length_for_text_value() {
        let backend = build_backend().await;

        let pool = backend.core.get_client().await.unwrap();
        sqlx::query("CREATE TABLE test_table (key TEXT PRIMARY KEY, value BLOB)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO test_table (key, value) VALUES ($1, $2)")
            .bind("key_id")
            .bind("你好")
            .execute(pool)
            .await
            .unwrap();

        let ctx = OperationContext::new();

        let rp = backend
            .stat(&ctx, "key_id", OpStat::default())
            .await
            .unwrap();
        assert_eq!(rp.into_metadata().content_length(), 6);

        let reader = backend.read(&ctx, "key_id", OpRead::default()).unwrap();
        let (rp, mut stream) = reader.open(BytesRange::from(0_u64..3)).await.unwrap();
        let buffer = stream.read_all().await.unwrap();

        assert_eq!(rp.into_metadata().unwrap().content_length(), 6);
        assert_eq!(buffer.to_vec(), "你".as_bytes());
    }

    #[tokio::test]
    async fn test_sqlite_stat_returns_byte_length_for_text_column() {
        let backend = build_backend().await;
        let pool = backend.core.get_client().await.unwrap();

        sqlx::query("CREATE TABLE test_table (key TEXT PRIMARY KEY, value TEXT)")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO test_table (key, value) VALUES ($1, $2)")
            .bind("key_id")
            .bind("你好")
            .execute(pool)
            .await
            .unwrap();

        let ctx = OperationContext::new();

        let rp = backend
            .stat(&ctx, "key_id", OpStat::default())
            .await
            .unwrap();
        assert_eq!(rp.into_metadata().content_length(), 6);

        let reader = backend.read(&ctx, "key_id", OpRead::default()).unwrap();
        let (rp, mut stream) = reader.open(BytesRange::from(0_u64..3)).await.unwrap();
        let buffer = stream.read_all().await.unwrap();

        assert_eq!(rp.into_metadata().unwrap().content_length(), 6);
        assert_eq!(buffer.to_vec(), "你".as_bytes());
    }
}
