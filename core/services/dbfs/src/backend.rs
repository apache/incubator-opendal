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

use bytes::Buf;
use http::StatusCode;
use log::debug;
use serde::Deserialize;

use super::DBFS_SCHEME;
use super::config::DbfsConfig;
use super::core::parse_error;
use super::core::{DbfsCore, ErrorContext};
use super::deleter::DbfsDeleter;
use super::lister::DbfsLister;
use super::writer::DbfsWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// [Dbfs](https://docs.databricks.com/api/azure/workspace/dbfs)'s REST API support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct DbfsBuilder {
    pub(super) config: DbfsConfig,
}

impl DbfsBuilder {
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

    /// Set endpoint of this backend.
    ///
    /// Endpoint must be full uri, e.g.
    ///
    /// - Azure: `https://adb-1234567890123456.78.azuredatabricks.net`
    /// - Aws: `https://dbc-123a5678-90bc.cloud.databricks.com`
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.config.endpoint = if endpoint.is_empty() {
            None
        } else {
            Some(endpoint.trim_end_matches('/').to_string())
        };
        self
    }

    /// Set the token of this backend.
    pub fn token(mut self, token: &str) -> Self {
        if !token.is_empty() {
            self.config.token = Some(token.to_string());
        }
        self
    }
}

impl Builder for DbfsBuilder {
    type Config = DbfsConfig;

    /// Build a DbfsBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let endpoint = match &self.config.endpoint {
            Some(endpoint) => Ok(endpoint.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", DBFS_SCHEME)),
        }?;
        debug!("backend use endpoint: {}", endpoint);

        let token = match self.config.token {
            Some(token) => token,
            None => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "missing token for Dbfs",
                ));
            }
        };

        let capability = Capability {
            stat: true,

            write: true,
            create_dir: true,
            delete: true,
            rename: true,

            list: true,

            shared: true,

            ..Default::default()
        };

        Ok(DbfsBackend {
            core: Arc::new(DbfsCore {
                root,
                endpoint: endpoint.to_string(),
                token,
            }),
            capability,
        })
    }
}

/// Backend for DBFS service
#[derive(Debug, Clone)]
pub struct DbfsBackend {
    core: Arc<DbfsCore>,
    capability: Capability,
}

impl Service for DbfsBackend {
    type Reader = ();
    type Writer = oio::OneShotWriter<DbfsWriter>;
    type Lister = oio::PageLister<DbfsLister>;
    type Deleter = oio::OneShotDeleter<DbfsDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        ServiceInfo::new(DBFS_SCHEME, &self.core.root, "")
    }

    fn capability(&self) -> Capability {
        self.capability
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let resp = self.core.dbfs_create_dir(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(RpCreateDir::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Mkdirs")),
                resp,
            )),
        }
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        // Stat root always returns a DIR.
        if path == "/" {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        let resp = self.core.dbfs_get_status(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let mut meta = MetadataBuilder::unknown();
                let bs = resp.into_body();
                let decoded_response: DbfsStatus =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                meta.last_modified(Timestamp::from_millisecond(
                    decoded_response.modification_time,
                )?);
                match decoded_response.is_dir {
                    true => meta.set_dir(),
                    false => meta.set_file(decoded_response.file_size as u64),
                };
                Ok(RpStat::new(meta.build()))
            }
            StatusCode::NOT_FOUND if path.ends_with('/') => {
                Ok(RpStat::new(MetadataBuilder::dir().build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetStatus")),
                resp,
            )),
        }
    }

    fn read(&self, _ctx: &OperationContext, _path: &str, _args: OpRead) -> Result<Self::Reader> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: oio::OneShotWriter<DbfsWriter> = {
            Ok(oio::OneShotWriter::new(DbfsWriter::new(
                self.core.clone(),
                ctx.clone(),
                args,
                path.to_string(),
            )))
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<DbfsDeleter> = {
            Ok(oio::OneShotDeleter::new(DbfsDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<DbfsLister> = {
            let l = DbfsLister::new(self.core.clone(), ctx.clone(), path.to_string());

            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        self.core.dbfs_ensure_parent_path(ctx, to).await?;

        let resp = self.core.dbfs_rename(ctx, from, to).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(RpRename::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Move")),
                resp,
            )),
        }
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

#[derive(Deserialize)]
struct DbfsStatus {
    // Not used fields.
    // path: String,
    is_dir: bool,
    file_size: i64,
    modification_time: i64,
}
