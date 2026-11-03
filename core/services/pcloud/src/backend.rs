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

use bytes::Buf;
use http::StatusCode;
use log::debug;
use opendal_core::raw::*;
use opendal_core::*;

use super::PCLOUD_SCHEME;
use super::config::PcloudConfig;
use super::core::PcloudError;
use super::core::parse_error;
use super::core::*;
use super::deleter::PcloudDeleter;
use super::lister::PcloudLister;
use super::reader::*;
use super::writer::PcloudWriter;
use super::writer::PcloudWriters;

/// [pCloud](https://www.pcloud.com/) services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct PcloudBuilder {
    pub(super) config: PcloudConfig,
}

impl Debug for PcloudBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcloudBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl PcloudBuilder {
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

    /// Pcloud endpoint.
    /// <https://api.pcloud.com> for United States and <https://eapi.pcloud.com> for Europe
    /// ref to [doc.pcloud.com](https://docs.pcloud.com/)
    ///
    /// It is required. e.g. `https://api.pcloud.com`
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.config.endpoint = endpoint.to_string();

        self
    }

    /// Pcloud username.
    ///
    /// It is required. your pCloud login email, e.g. `example@gmail.com`
    pub fn username(mut self, username: &str) -> Self {
        self.config.username = if username.is_empty() {
            None
        } else {
            Some(username.to_string())
        };

        self
    }

    /// Pcloud password.
    ///
    /// It is required. your pCloud login password, e.g. `password`
    pub fn password(mut self, password: &str) -> Self {
        self.config.password = if password.is_empty() {
            None
        } else {
            Some(password.to_string())
        };

        self
    }
}

impl Builder for PcloudBuilder {
    type Config = PcloudConfig;

    /// Builds the backend and returns the result of PcloudBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", root);

        // Handle endpoint.
        if self.config.endpoint.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", PCLOUD_SCHEME));
        }

        debug!("backend use endpoint {}", self.config.endpoint);

        let username = match &self.config.username {
            Some(username) => Ok(username.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "username is empty")
                .with_operation("Builder::build")
                .with_context("service", PCLOUD_SCHEME)),
        }?;

        let password = match &self.config.password {
            Some(password) => Ok(password.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "password is empty")
                .with_operation("Builder::build")
                .with_context("service", PCLOUD_SCHEME)),
        }?;

        Ok(PcloudBackend {
            core: Arc::new(PcloudCore {
                info: ServiceInfo::new(PCLOUD_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,

                    create_dir: true,

                    read: true,
                    read_with_suffix: true,

                    write: true,

                    delete: true,
                    rename: true,
                    copy: true,

                    list: true,

                    shared: true,

                    ..Default::default()
                },
                root,
                endpoint: self.config.endpoint.clone(),
                username,
                password,
            }),
        })
    }
}

/// Backend for Pcloud services.
#[derive(Debug, Clone)]
pub struct PcloudBackend {
    pub(crate) core: Arc<PcloudCore>,
}

impl Service for PcloudBackend {
    type Reader = oio::StreamReader<PcloudReader>;
    type Writer = PcloudWriters;
    type Lister = oio::PageLister<PcloudLister>;
    type Deleter = oio::OneShotDeleter<PcloudDeleter>;
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
        ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.core.ensure_dir_exists(ctx, path).await?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let resp = self.core.stat(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();
                let resp: StatResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                let result = resp.result;
                if result == 2010 || result == 2055 || result == 2002 {
                    return Err(Error::new(ErrorKind::NotFound, format!("{resp:?}")));
                }
                if result != 0 {
                    return Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")));
                }

                if let Some(md) = resp.metadata {
                    let md = parse_stat_metadata(md);
                    return md.map(RpStat::new);
                }

                Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Stat")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<PcloudReader> = {
            Ok(oio::StreamReader::new(PcloudReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, _args: OpWrite) -> Result<Self::Writer> {
        let output: PcloudWriters = {
            let writer = PcloudWriter::new(self.core.clone(), ctx.clone(), path.to_string());

            let w = oio::OneShotWriter::new(writer);

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<PcloudDeleter> = {
            Ok(oio::OneShotDeleter::new(PcloudDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<PcloudLister> = {
            let l = PcloudLister::new(self.core.clone(), ctx.clone(), path);
            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let backend = self.clone();
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();
        let source_content_length_hint = args.source_content_length_hint();

        Ok(oio::OneShotCopier::new(async move {
            let source_size = if from.ends_with('/') {
                None
            } else {
                Some(match source_content_length_hint {
                    Some(size) => size,
                    None => backend
                        .stat(&ctx, &from, OpStat::default())
                        .await?
                        .into_metadata()
                        .content_length(),
                })
            };

            core.ensure_dir_exists(&ctx, &to).await?;

            let resp = if from.ends_with('/') {
                core.copy_folder(&ctx, &from, &to).await?
            } else {
                core.copy_file(&ctx, &from, &to).await?
            };

            let status = resp.status();

            match status {
                StatusCode::OK => {
                    let bs = resp.into_body();
                    let resp: PcloudError =
                        serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                    let result = resp.result;
                    if result == 2009 || result == 2010 || result == 2055 || result == 2002 {
                        Err(Error::new(ErrorKind::NotFound, format!("{resp:?}")))
                    } else if result != 0 {
                        Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")))
                    } else {
                        let metadata =
                            source_size.map_or_else(MetadataBuilder::dir, MetadataBuilder::file);
                        Ok(metadata.build())
                    }
                }
                _ => Err(parse_error(
                    ErrorContext::new(if from.ends_with('/') {
                        ServiceOperation("CopyFolder")
                    } else {
                        ServiceOperation("CopyFile")
                    }),
                    resp,
                )),
            }
        }))
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        self.core.ensure_dir_exists(ctx, to).await?;

        let resp = if from.ends_with('/') {
            self.core.rename_folder(ctx, from, to).await?
        } else {
            self.core.rename_file(ctx, from, to).await?
        };

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();
                let resp: PcloudError =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                let result = resp.result;
                if result == 2009 || result == 2010 || result == 2055 || result == 2002 {
                    return Err(Error::new(ErrorKind::NotFound, format!("{resp:?}")));
                }
                if result != 0 {
                    return Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")));
                }

                Ok(RpRename::default())
            }
            _ => Err(parse_error(
                ErrorContext::new(if from.ends_with('/') {
                    ServiceOperation("RenameFolder")
                } else {
                    ServiceOperation("RenameFile")
                }),
                resp,
            )),
        }
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
