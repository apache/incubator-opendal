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

use asyncband::mutex::Mutex;
use asyncband::once::OnceCell;
use bytes::Buf;
use http::StatusCode;
use log::debug;

use super::KOOFR_SCHEME;
use super::config::KoofrConfig;
use super::core::File;
use super::core::KoofrSigner;
use super::core::parse_error;
use super::core::{ErrorContext, KoofrCore};
use super::deleter::KoofrDeleter;
use super::lister::KoofrLister;
use super::reader::*;
use super::writer::KoofrWriter;
use super::writer::KoofrWriters;
use opendal_core::raw::*;
use opendal_core::*;

/// [Koofr](https://app.koofr.net/) services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct KoofrBuilder {
    pub(super) config: KoofrConfig,
}

impl Debug for KoofrBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KoofrBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl KoofrBuilder {
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

    /// endpoint.
    ///
    /// It is required. e.g. `https://api.koofr.net/`
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.config.endpoint = endpoint.to_string();

        self
    }

    /// email.
    ///
    /// It is required. e.g. `test@example.com`
    pub fn email(mut self, email: &str) -> Self {
        self.config.email = email.to_string();

        self
    }

    /// Koofr application password.
    ///
    /// Go to <https://app.koofr.net/app/admin/preferences/password>.
    /// Click "Generate Password" button to generate a new application password.
    ///
    /// # Notes
    ///
    /// This is not user's Koofr account password.
    /// Please use the application password instead.
    /// Please also remind users of this.
    pub fn password(mut self, password: &str) -> Self {
        self.config.password = if password.is_empty() {
            None
        } else {
            Some(password.to_string())
        };

        self
    }
}

impl Builder for KoofrBuilder {
    type Config = KoofrConfig;

    /// Builds the backend and returns the result of KoofrBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", root);

        if self.config.endpoint.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", KOOFR_SCHEME));
        }

        debug!("backend use endpoint {}", self.config.endpoint);

        if self.config.email.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "email is empty")
                .with_operation("Builder::build")
                .with_context("service", KOOFR_SCHEME));
        }

        debug!("backend use email {}", self.config.email);

        let password = match &self.config.password {
            Some(password) => Ok(password.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "password is empty")
                .with_operation("Builder::build")
                .with_context("service", KOOFR_SCHEME)),
        }?;

        let signer = Arc::new(Mutex::new(KoofrSigner::default()));

        Ok(KoofrBackend {
            core: Arc::new(KoofrCore {
                info: ServiceInfo::new(KOOFR_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,

                    create_dir: true,

                    read: true,
                    read_with_suffix: true,

                    write: true,
                    write_can_empty: true,

                    delete: true,

                    rename: true,

                    copy: true,

                    list: true,

                    shared: true,

                    ..Default::default()
                },
                root,
                endpoint: self.config.endpoint.clone(),
                email: self.config.email.clone(),
                password,
                mount_id: OnceCell::new(),
                signer,
            }),
        })
    }
}

/// Backend for Koofr services.
#[derive(Debug, Clone)]
pub struct KoofrBackend {
    pub(crate) core: Arc<KoofrCore>,
}

impl Service for KoofrBackend {
    type Reader = oio::StreamReader<KoofrReader>;
    type Writer = KoofrWriters;
    type Lister = oio::PageLister<KoofrLister>;
    type Deleter = oio::OneShotDeleter<KoofrDeleter>;
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
        self.core
            .create_dir(ctx, &build_abs_path(&self.core.root, path))
            .await?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let path = build_rooted_abs_path(&self.core.root, path);
        let resp = self.core.info(ctx, &path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let file: File =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                let mode = if file.ty == "dir" {
                    EntryMode::DIR
                } else {
                    EntryMode::FILE
                };

                let mut md = if mode == EntryMode::FILE {
                    MetadataBuilder::file(file.size)
                } else {
                    MetadataBuilder::dir()
                };

                md.content_type(&file.content_type)
                    .last_modified(Timestamp::from_millisecond(file.modified)?);

                Ok(RpStat::new(md.build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("FilesInfo")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<KoofrReader> = {
            Ok(oio::StreamReader::new(KoofrReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, _args: OpWrite) -> Result<Self::Writer> {
        let output: KoofrWriters = {
            let writer = KoofrWriter::new(self.core.clone(), ctx.clone(), path.to_string());

            let w = oio::OneShotWriter::new(writer);

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<KoofrDeleter> = {
            Ok(oio::OneShotDeleter::new(KoofrDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<KoofrLister> = {
            let l = KoofrLister::new(self.core.clone(), ctx.clone(), path);
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
            let source_size = match source_content_length_hint {
                Some(size) => size,
                None => backend
                    .stat(&ctx, &from, OpStat::default())
                    .await?
                    .into_metadata()
                    .content_length(),
            };

            core.ensure_dir_exists(&ctx, &to).await?;
            if from == to {
                Ok(MetadataBuilder::file(source_size).build())
            } else {
                let resp = core.remove(&ctx, &to).await?;

                let status = resp.status();

                if status != StatusCode::OK && status != StatusCode::NOT_FOUND {
                    Err(parse_error(
                        ErrorContext::new(ServiceOperation("FilesRemove")),
                        resp,
                    ))
                } else {
                    let resp = core.copy(&ctx, &from, &to).await?;

                    let status = resp.status();

                    match status {
                        StatusCode::OK => Ok(MetadataBuilder::file(source_size).build()),
                        _ => Err(parse_error(
                            ErrorContext::new(ServiceOperation("FilesCopy")),
                            resp,
                        )),
                    }
                }
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

        if from == to {
            return Ok(RpRename::default());
        }

        let resp = self.core.remove(ctx, to).await?;

        let status = resp.status();

        if status != StatusCode::OK && status != StatusCode::NOT_FOUND {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("FilesRemove")),
                resp,
            ));
        }

        let resp = self.core.move_object(ctx, from, to).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(RpRename::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("FilesMove")),
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
