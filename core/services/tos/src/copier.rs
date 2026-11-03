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

use crate::core::parse_error;
use crate::core::tos_parse_into_metadata;
use crate::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub type TosCopiers = oio::MultipartCopier<TosCopier>;

pub fn new_tos_copier(
    core: Arc<TosCore>,
    ctx: &OperationContext,
    from: &str,
    to: &str,
    args: OpCopy,
) -> Result<TosCopiers> {
    let capability = core.capability;
    let max_part_size = capability.copy_multi_max_size.ok_or_else(|| {
        Error::new(
            ErrorKind::Unexpected,
            "multipart copy requires copy_multi_max_size capability",
        )
    })?;

    let (copy_once_threshold, part_size) = match args.chunk() {
        Some(chunk) => {
            let min_part_size = capability.copy_multi_min_size.ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "multipart copy requires copy_multi_min_size capability",
                )
            })?;
            let part_size = chunk.clamp(min_part_size, max_part_size) as u64;
            (part_size.saturating_sub(1), part_size)
        }
        None => {
            let part_size = max_part_size as u64;
            (part_size, part_size)
        }
    };
    let source_content_length_hint = args.source_content_length_hint();
    let concurrent = args.concurrent();

    Ok(oio::MultipartCopier::new(
        (ctx.executor().clone(), capability),
        TosCopier {
            core,
            ctx: ctx.clone(),
            from: from.to_string(),
            to: to.to_string(),
            args,
        },
        source_content_length_hint,
        copy_once_threshold,
        part_size,
        concurrent,
    ))
}

pub struct TosCopier {
    core: Arc<TosCore>,
    ctx: OperationContext,
    from: String,
    to: String,
    args: OpCopy,
}

impl oio::MultipartCopy for TosCopier {
    async fn source_metadata(&self) -> Result<Metadata> {
        let resp = self
            .core
            .tos_head_object(&self.ctx, &self.from, OpStat::default())
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let headers = resp.headers();
                tos_parse_into_metadata(&self.from, headers)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("HeadObject")),
                resp,
            )),
        }
    }

    async fn copy_once(&self) -> Result<Metadata> {
        let resp = self
            .core
            .tos_copy_object(&self.ctx, &self.from, &self.to, &self.args)
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let result: CopyObjectOutput = serde_json::from_reader(resp.into_body().reader())
                    .map_err(new_json_deserialize_error)?;
                if result.etag.is_empty() {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        "ETag not present in copy response",
                    )
                    .set_temporary());
                }

                let mut meta = if self.to.ends_with('/') {
                    MetadataBuilder::dir()
                } else {
                    MetadataBuilder::unknown()
                };
                meta.etag(result.etag.trim_matches('"'));
                Ok(meta.build())
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CopyObject")),
                resp,
            )),
        }
    }

    async fn initiate_copy(&self) -> Result<String> {
        let resp = self
            .core
            .tos_initiate_multipart_copy(&self.ctx, &self.to)
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let result: InitiateMultipartUploadResult =
                    serde_json::from_reader(resp.into_body().reader())
                        .map_err(new_json_deserialize_error)?;

                Ok(result.upload_id)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateMultipartUpload")),
                resp,
            )),
        }
    }

    async fn copy_part(
        &self,
        upload_id: &str,
        part_number: usize,
        range: BytesRange,
    ) -> Result<oio::MultipartPart> {
        let size = range.size().expect("multipart copy range must be sized");
        let part_number = part_number + 1;

        let req = self
            .core
            .tos_upload_part_copy_request(TosUploadPartCopyRequest {
                from: &self.from,
                to: &self.to,
                upload_id,
                part_number,
                range,
            })?;
        let resp = self.core.send(&self.ctx, req).await?;

        match resp.status() {
            StatusCode::OK => {
                let result: UploadPartCopyOutput =
                    serde_json::from_reader(resp.into_body().reader())
                        .map_err(new_json_deserialize_error)?;
                if result.etag.is_empty() {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        "ETag not present in copy part response",
                    )
                    .set_temporary());
                }

                Ok(oio::MultipartPart {
                    part_number,
                    etag: result.etag.trim_matches('"').to_string(),
                    checksum: None,
                    size: Some(size),
                })
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadPartCopy")),
                resp,
            )),
        }
    }

    async fn complete_copy(
        &self,
        upload_id: &str,
        parts: &[oio::MultipartPart],
    ) -> Result<Metadata> {
        let parts = parts
            .iter()
            .map(|p| CompleteMultipartUploadRequestPart {
                part_number: p.part_number,
                etag: p.etag.clone(),
            })
            .collect();

        let resp = self
            .core
            .tos_complete_multipart_copy(&self.ctx, &self.to, upload_id, parts, &self.args)
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let ret: CompleteMultipartUploadResult =
                    serde_json::from_reader(resp.into_body().reader())
                        .map_err(new_json_deserialize_error)?;
                if !ret.code.is_empty() {
                    return Err(Error::new(ErrorKind::Unexpected, ret.message));
                }

                let mut meta = if self.to.ends_with('/') {
                    MetadataBuilder::dir()
                } else {
                    MetadataBuilder::unknown()
                };
                if !ret.etag.is_empty() {
                    meta.etag(ret.etag.trim_matches('"'));
                }
                if !ret.version_id.is_empty() {
                    meta.version(&ret.version_id);
                }

                Ok(meta.build())
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CompleteMultipartUpload")),
                resp,
            )),
        }
    }

    async fn abort_copy(&self, upload_id: &str) -> Result<()> {
        let resp = self
            .core
            .tos_abort_multipart_copy(&self.ctx, &self.to, upload_id)
            .await?;
        match resp.status() {
            StatusCode::NO_CONTENT => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("AbortMultipartUpload")),
                resp,
            )),
        }
    }
}
