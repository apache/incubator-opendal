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

use super::YANDEX_DISK_SCHEME;
use super::config::YandexDiskConfig;
use super::core::parse_error;
use super::core::*;
use super::deleter::YandexDiskDeleter;
use super::lister::YandexDiskLister;
use super::reader::*;
use super::writer::YandexDiskWriter;
use super::writer::YandexDiskWriters;
use opendal_core::raw::*;
use opendal_core::*;

/// [YandexDisk](https://360.yandex.com/disk/) services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct YandexDiskBuilder {
    pub(super) config: YandexDiskConfig,
}

impl Debug for YandexDiskBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YandexDiskBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl YandexDiskBuilder {
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

    /// yandex disk oauth access_token.
    /// The valid token will looks like `y0_XXXXXXqihqIWAADLWwAAAAD3IXXXXXX0gtVeSPeIKM0oITMGhXXXXXX`.
    /// We can fetch the debug token from <https://yandex.com/dev/disk/poligon>.
    /// To use it in production, please register an app at <https://oauth.yandex.com> instead.
    pub fn access_token(mut self, access_token: &str) -> Self {
        self.config.access_token = access_token.to_string();

        self
    }
}

impl Builder for YandexDiskBuilder {
    type Config = YandexDiskConfig;

    /// Builds the backend and returns the result of YandexDiskBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", root);

        // Handle oauth access_token.
        if self.config.access_token.is_empty() {
            return Err(
                Error::new(ErrorKind::ConfigInvalid, "access_token is empty")
                    .with_operation("Builder::build")
                    .with_context("service", YANDEX_DISK_SCHEME),
            );
        }

        Ok(YandexDiskBackend {
            core: Arc::new(YandexDiskCore {
                info: ServiceInfo::new(YANDEX_DISK_SCHEME, &root, ""),
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
                    list_with_limit: true,

                    shared: true,

                    ..Default::default()
                },
                root,
                access_token: self.config.access_token.clone(),
            }),
        })
    }
}

/// Backend for YandexDisk services.
#[derive(Debug, Clone)]
pub struct YandexDiskBackend {
    pub(crate) core: Arc<YandexDiskCore>,
}

impl Service for YandexDiskBackend {
    type Reader = oio::StreamReader<YandexDiskReader>;
    type Writer = YandexDiskWriters;
    type Lister = oio::PageLister<YandexDiskLister>;
    type Deleter = oio::OneShotDeleter<YandexDiskDeleter>;
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

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        self.core.ensure_dir_exists(ctx, to).await?;

        let resp = self.core.move_object(ctx, from, to).await?;

        let status = resp.status();

        match status {
            StatusCode::OK | StatusCode::CREATED => Ok(RpRename::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("MoveResource")),
                resp,
            )),
        }
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

            let resp = core.copy(&ctx, &from, &to).await?;

            let status = resp.status();

            match status {
                StatusCode::OK | StatusCode::CREATED => {
                    Ok(MetadataBuilder::file(source_size).build())
                }
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyResource")),
                    resp,
                )),
            }
        }))
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<YandexDiskReader> = {
            Ok(oio::StreamReader::new(YandexDiskReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let resp = self.core.metainformation(ctx, path, None, None).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let mf: MetainformationResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                parse_info(mf).map(RpStat::new)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetMetainformation")),
                resp,
            )),
        }
    }

    fn write(&self, ctx: &OperationContext, path: &str, _args: OpWrite) -> Result<Self::Writer> {
        let output: YandexDiskWriters = {
            let writer = YandexDiskWriter::new(self.core.clone(), ctx.clone(), path.to_string());

            let w = oio::OneShotWriter::new(writer);

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<YandexDiskDeleter> = {
            Ok(oio::OneShotDeleter::new(YandexDiskDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<YandexDiskLister> = {
            let l = YandexDiskLister::new(self.core.clone(), ctx.clone(), path, args.limit());
            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
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
