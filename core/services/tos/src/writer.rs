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
use constants::X_TOS_OBJECT_SIZE;
use constants::X_TOS_VERSION_ID;
use http::StatusCode;

use crate::core::parse_error;
use crate::core::tos_parse_etag;
use crate::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub struct TosWriter {
    core: Arc<TosCore>,
    ctx: OperationContext,

    op: OpWrite,
    path: String,
}

impl TosWriter {
    pub fn new(core: Arc<TosCore>, ctx: OperationContext, path: &str, op: OpWrite) -> Self {
        TosWriter {
            core,
            ctx,
            path: path.to_string(),
            op,
        }
    }

    fn parse_header_into_meta(path: &str, headers: &http::HeaderMap) -> Result<Metadata> {
        let mut meta = if path.ends_with('/') {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::unknown()
        };
        if let Some(etag) = tos_parse_etag(headers)? {
            meta.etag(etag);
        }
        if let Some(version) = parse_header_to_str(headers, X_TOS_VERSION_ID)? {
            meta.version(version);
        }
        if !path.ends_with('/')
            && let Some(value) =
                parse_header_to_str(headers, X_TOS_OBJECT_SIZE)?.and_then(|size| size.parse().ok())
        {
            meta.set_file(value);
        }
        Ok(meta.build())
    }
}

impl oio::MultipartWrite for TosWriter {
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        let req = self
            .core
            .tos_put_object_request(&self.path, Some(size), &self.op, body)?;

        let resp = self.core.send(&self.ctx, req).await?;

        let status = resp.status();

        let meta = TosWriter::parse_header_into_meta(&self.path, resp.headers())?;

        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(meta),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("PutObject")),
                resp,
            )),
        }
    }

    async fn initiate_part(&self) -> Result<String> {
        let resp = self
            .core
            .tos_initiate_multipart_upload(&self.ctx, &self.path, &self.op)
            .await?;

        let status = resp.status();

        match status {
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

    async fn write_part(
        &self,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<oio::MultipartPart> {
        let part_number = part_number + 1;

        let req =
            self.core
                .tos_upload_part_request(&self.path, upload_id, part_number, size, body)?;

        let resp = self.core.send(&self.ctx, req).await?;
        let status = resp.status();

        match status {
            StatusCode::OK => {
                let etag = tos_parse_etag(resp.headers())?
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::Unexpected,
                            "ETag not present in returning response",
                        )
                    })?
                    .to_string();

                Ok(oio::MultipartPart {
                    part_number,
                    etag,
                    checksum: None,
                    size: None,
                })
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadPart")),
                resp,
            )),
        }
    }

    async fn complete_part(
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
            .tos_complete_multipart_upload(&self.ctx, &self.path, upload_id, parts, &self.op)
            .await?;

        let status = resp.status();
        let mut meta =
            TosWriter::parse_header_into_meta(&self.path, resp.headers())?.into_builder();

        match status {
            StatusCode::OK => {
                let ret: CompleteMultipartUploadResult =
                    serde_json::from_reader(resp.into_body().reader())
                        .map_err(new_json_deserialize_error)?;
                if !ret.code.is_empty() {
                    return Err(Error::new(ErrorKind::Unexpected, ret.message));
                }
                if !ret.etag.is_empty() {
                    // CompleteMultipartUpload response wraps ETag in quotes:
                    // https://www.volcengine.com/docs/6349/74868
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

    async fn abort_part(&self, upload_id: &str) -> Result<()> {
        let resp = self
            .core
            .tos_abort_multipart_upload(&self.ctx, &self.path, upload_id)
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
