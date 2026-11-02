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

use asyncband::rwlock::RwLock;
use http::Request;
use http::StatusCode;
use log::debug;

use super::B2_SCHEME;
use super::config::B2Config;
use super::core::B2Signer;
use super::core::constants;
use super::core::parse_error;
use super::core::parse_file_info;
use super::core::{B2Core, ErrorContext};
use super::deleter::B2Deleter;
use super::lister::B2Lister;
use super::reader::*;
use super::writer::B2Writer;
use super::writer::B2Writers;
use opendal_core::raw::*;
use opendal_core::*;

/// [b2](https://www.backblaze.com/cloud-storage) services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct B2Builder {
    pub(super) config: B2Config,
}

impl Debug for B2Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("B2Builder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl B2Builder {
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

    /// application_key_id of this backend.
    pub fn application_key_id(mut self, application_key_id: &str) -> Self {
        self.config.application_key_id = if application_key_id.is_empty() {
            None
        } else {
            Some(application_key_id.to_string())
        };

        self
    }

    /// application_key of this backend.
    pub fn application_key(mut self, application_key: &str) -> Self {
        self.config.application_key = if application_key.is_empty() {
            None
        } else {
            Some(application_key.to_string())
        };

        self
    }

    /// Set bucket name of this backend.
    /// You can find it in <https://secure.backblaze.com/b2_buckets.html>
    pub fn bucket(mut self, bucket: &str) -> Self {
        self.config.bucket = bucket.to_string();

        self
    }

    /// Set bucket id of this backend.
    /// You can find it in <https://secure.backblaze.com/b2_buckets.html>
    pub fn bucket_id(mut self, bucket_id: &str) -> Self {
        self.config.bucket_id = bucket_id.to_string();

        self
    }
}

impl Builder for B2Builder {
    type Config = B2Config;

    /// Builds the backend and returns the result of B2Backend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", root);

        // Handle bucket.
        if self.config.bucket.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "bucket is empty")
                .with_operation("Builder::build")
                .with_context("service", B2_SCHEME));
        }

        debug!("backend use bucket {}", self.config.bucket);

        // Handle bucket_id.
        if self.config.bucket_id.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "bucket_id is empty")
                .with_operation("Builder::build")
                .with_context("service", B2_SCHEME));
        }

        debug!("backend bucket_id {}", self.config.bucket_id);

        let application_key_id = match &self.config.application_key_id {
            Some(application_key_id) => Ok(application_key_id.clone()),
            None => Err(
                Error::new(ErrorKind::ConfigInvalid, "application_key_id is empty")
                    .with_operation("Builder::build")
                    .with_context("service", B2_SCHEME),
            ),
        }?;

        let application_key = match &self.config.application_key {
            Some(key_id) => Ok(key_id.clone()),
            None => Err(
                Error::new(ErrorKind::ConfigInvalid, "application_key is empty")
                    .with_operation("Builder::build")
                    .with_context("service", B2_SCHEME),
            ),
        }?;

        let signer = B2Signer {
            application_key_id,
            application_key,
            ..Default::default()
        };

        Ok(B2Backend {
            core: Arc::new(B2Core {
                info: ServiceInfo::new(B2_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,

                    read: true,
                    read_with_suffix: true,

                    write: true,
                    write_can_empty: true,
                    write_can_multi: true,
                    write_with_content_type: true,
                    write_with_user_metadata: true,
                    // The min multipart size of b2 is 5 MiB.
                    //
                    // ref: <https://www.backblaze.com/docs/cloud-storage-large-files>
                    write_multi_min_size: Some(5 * 1024 * 1024),
                    // The max multipart size of b2 is 5 Gb.
                    //
                    // ref: <https://www.backblaze.com/docs/cloud-storage-large-files>
                    write_multi_max_size: if cfg!(target_pointer_width = "64") {
                        Some(5 * 1024 * 1024 * 1024)
                    } else {
                        Some(usize::MAX)
                    },

                    delete: true,
                    copy: true,

                    list: true,
                    list_with_limit: true,
                    list_with_start_after: true,
                    list_with_recursive: true,

                    presign: true,
                    presign_read: true,
                    presign_write: true,
                    presign_stat: true,

                    shared: true,

                    ..Default::default()
                },
                signer: Arc::new(RwLock::new(signer)),
                root,

                bucket: self.config.bucket.clone(),
                bucket_id: self.config.bucket_id.clone(),
            }),
        })
    }
}

/// Backend for b2 services.
#[derive(Debug, Clone)]
pub struct B2Backend {
    pub(crate) core: Arc<B2Core>,
}

impl Service for B2Backend {
    type Reader = oio::StreamReader<B2Reader>;
    type Writer = B2Writers;
    type Lister = oio::PageLister<B2Lister>;
    type Deleter = oio::OneShotDeleter<B2Deleter>;
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
        _ctx: &OperationContext,
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    /// B2 have a get_file_info api required a file_id field, but field_id need call list api, list api also return file info
    /// So we call list api to get file info
    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        // Stat root always returns a DIR.
        if path == "/" {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        let delimiter = if path.ends_with('/') { Some("/") } else { None };

        let file_info = self.core.get_file_info(ctx, path, delimiter).await?;
        let meta = parse_file_info(&file_info);
        Ok(RpStat::new(meta))
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<B2Reader> = {
            Ok(oio::StreamReader::new(B2Reader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: B2Writers = {
            let concurrent = args.concurrent();
            let writer = B2Writer::new(self.core.clone(), ctx.clone(), path, args);

            let w = oio::MultipartWriter::new(ctx.executor().clone(), writer, concurrent);

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<B2Deleter> = {
            Ok(oio::OneShotDeleter::new(B2Deleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<B2Lister> = {
            Ok(oio::PageLister::new(B2Lister::new(
                self.core.clone(),
                ctx.clone(),
                path,
                args.recursive(),
                args.limit(),
                args.start_after(),
            )))
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _args: OpCopy,
    ) -> Result<Self::Copier> {
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();

        Ok(oio::OneShotCopier::new(async move {
            let file_info = core.get_file_info(&ctx, &from, None).await?;
            let source_file_id = file_info.file_id;
            let source_content_length = file_info.content_length;

            let Some(source_file_id) = source_file_id else {
                return Err(Error::new(ErrorKind::IsADirectory, "is a directory"));
            };

            let resp = core.copy_file(&ctx, source_file_id, &to).await?;

            let status = resp.status();

            match status {
                StatusCode::OK => {
                    let metadata = MetadataBuilder::file(source_content_length);
                    Ok(metadata.build())
                }
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyFile")),
                    resp,
                )),
            }
        }))
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
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        match args.operation() {
            PresignOperation::Stat(_) => {
                let resp = self
                    .core
                    .get_download_authorization(ctx, path, args.expire())
                    .await?;
                let path = build_abs_path(&self.core.root, path);

                let auth_info = self.core.get_auth_info(ctx).await?;

                let url = format!(
                    "{}/file/{}/{}?Authorization={}",
                    auth_info.download_url,
                    self.core.bucket,
                    percent_encode_path(&path),
                    resp.authorization_token
                );

                let req = Request::get(url);

                let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

                // We don't need this request anymore, consume
                let (parts, _) = req.into_parts();

                Ok(RpPresign::new(PresignedRequest::new(
                    parts.method,
                    parts.uri,
                    parts.headers,
                )))
            }
            PresignOperation::Read(range, _) => {
                let resp = self
                    .core
                    .get_download_authorization(ctx, path, args.expire())
                    .await?;
                let path = build_abs_path(&self.core.root, path);

                let auth_info = self.core.get_auth_info(ctx).await?;

                let url = format!(
                    "{}/file/{}/{}?Authorization={}",
                    auth_info.download_url,
                    self.core.bucket,
                    percent_encode_path(&path),
                    resp.authorization_token
                );

                let mut req = Request::get(url);
                if !range.is_full() {
                    req = req.header(http::header::RANGE, range.to_header());
                }

                let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

                // We don't need this request anymore, consume
                let (parts, _) = req.into_parts();

                Ok(RpPresign::new(PresignedRequest::new(
                    parts.method,
                    parts.uri,
                    parts.headers,
                )))
            }
            PresignOperation::Write(_) => {
                let resp = self.core.get_upload_url(ctx).await?;

                let mut req = Request::post(&resp.upload_url);

                req = req.header(http::header::AUTHORIZATION, resp.authorization_token);
                req = req.header(
                    "X-Bz-File-Name",
                    percent_encode_path(&build_abs_path(&self.core.root, path)),
                );
                req = req.header(http::header::CONTENT_TYPE, "b2/x-auto");
                req = req.header(constants::X_BZ_CONTENT_SHA1, "do_not_verify");

                let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
                // We don't need this request anymore, consume it directly.
                let (parts, _) = req.into_parts();

                Ok(RpPresign::new(PresignedRequest::new(
                    parts.method,
                    parts.uri,
                    parts.headers,
                )))
            }
            PresignOperation::Delete(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            )),
            _ => Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            )),
        }
    }
}
