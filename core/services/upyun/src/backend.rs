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

use http::StatusCode;
use log::debug;
use opendal_core::raw::*;
use opendal_core::*;

use super::UPYUN_SCHEME;
use super::config::UpyunConfig;
use super::core::parse_error;
use super::core::*;
use super::deleter::UpyunDeleter;
use super::lister::UpyunLister;
use super::reader::*;
use super::writer::UpyunWriter;
use super::writer::UpyunWriters;

/// [upyun](https://www.upyun.com/products/file-storage) services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct UpyunBuilder {
    pub(super) config: UpyunConfig,
}

impl Debug for UpyunBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpyunBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl UpyunBuilder {
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

    /// bucket of this backend.
    ///
    /// It is required. e.g. `test`
    pub fn bucket(mut self, bucket: &str) -> Self {
        self.config.bucket = bucket.to_string();

        self
    }

    /// operator of this backend.
    ///
    /// It is required. e.g. `test`
    pub fn operator(mut self, operator: &str) -> Self {
        self.config.operator = if operator.is_empty() {
            None
        } else {
            Some(operator.to_string())
        };

        self
    }

    /// password of this backend.
    ///
    /// It is required. e.g. `asecret`
    pub fn password(mut self, password: &str) -> Self {
        self.config.password = if password.is_empty() {
            None
        } else {
            Some(password.to_string())
        };

        self
    }
}

impl Builder for UpyunBuilder {
    type Config = UpyunConfig;

    /// Builds the backend and returns the result of UpyunBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", root);

        // Handle bucket.
        if self.config.bucket.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "bucket is empty")
                .with_operation("Builder::build")
                .with_context("service", UPYUN_SCHEME));
        }

        debug!("backend use bucket {}", self.config.bucket);

        let operator = match &self.config.operator {
            Some(operator) => Ok(operator.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "operator is empty")
                .with_operation("Builder::build")
                .with_context("service", UPYUN_SCHEME)),
        }?;

        let password = match &self.config.password {
            Some(password) => Ok(password.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "password is empty")
                .with_operation("Builder::build")
                .with_context("service", UPYUN_SCHEME)),
        }?;

        let signer = UpyunSigner {
            operator: operator.clone(),
            password: password.clone(),
        };

        Ok(UpyunBackend {
            core: Arc::new(UpyunCore {
                info: ServiceInfo::new(UPYUN_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,

                    create_dir: true,

                    read: true,
                    read_with_suffix: true,

                    write: true,
                    write_can_empty: true,
                    write_can_multi: true,
                    write_with_cache_control: true,
                    write_with_content_type: true,

                    // https://help.upyun.com/knowledge-base/rest_api/#e5b9b6e8a18ce5bc8fe696ade782b9e7bbade4bca0
                    write_multi_min_size: Some(1024 * 1024),
                    write_multi_max_size: Some(50 * 1024 * 1024),

                    delete: true,
                    rename: true,
                    copy: true,

                    list: true,
                    list_with_limit: true,

                    shared: true,

                    ..Default::default()
                },
                root,
                operator,
                bucket: self.config.bucket.clone(),
                signer,
            }),
        })
    }
}

/// Backend for upyun services.
#[derive(Debug, Clone)]
pub struct UpyunBackend {
    pub(crate) core: Arc<UpyunCore>,
}

impl Service for UpyunBackend {
    type Reader = oio::StreamReader<UpyunReader>;
    type Writer = UpyunWriters;
    type Lister = oio::PageLister<UpyunLister>;
    type Deleter = oio::OneShotDeleter<UpyunDeleter>;
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
        let resp = self.core.create_dir(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(RpCreateDir::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateFolder")),
                resp,
            )),
        }
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let resp = self.core.info(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => parse_info(resp.headers()).map(RpStat::new),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetFileInfo")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<UpyunReader> = {
            Ok(oio::StreamReader::new(UpyunReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: UpyunWriters = {
            let concurrent = args.concurrent();
            let writer = UpyunWriter::new(self.core.clone(), ctx.clone(), args, path.to_string());

            let w = oio::MultipartWriter::new(ctx.executor().clone(), writer, concurrent);

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<UpyunDeleter> = {
            Ok(oio::OneShotDeleter::new(UpyunDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<UpyunLister> = {
            let l = UpyunLister::new(self.core.clone(), ctx.clone(), path, args.limit());
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

            let resp = core.copy(&ctx, &from, &to).await?;
            let status = resp.status();

            match status {
                StatusCode::OK => Ok(MetadataBuilder::file(source_size).build()),
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyFile")),
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
        let resp = self.core.move_object(ctx, from, to).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(RpRename::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("MoveFile")),
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
