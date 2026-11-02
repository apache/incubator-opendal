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

use super::core::StartLargeFileResponse;
use super::core::UploadPartResponse;
use super::core::UploadResponse;
use super::core::parse_error;
use super::core::{B2Core, ErrorContext};
use opendal_core::raw::*;
use opendal_core::*;

pub type B2Writers = oio::MultipartWriter<B2Writer>;

pub struct B2Writer {
    core: Arc<B2Core>,
    ctx: OperationContext,

    op: OpWrite,
    path: String,
}

impl B2Writer {
    pub fn new(core: Arc<B2Core>, ctx: OperationContext, path: &str, op: OpWrite) -> Self {
        B2Writer {
            core,
            ctx,
            path: path.to_string(),
            op,
        }
    }

    pub fn parse_body_into_meta(path: &str, resp: UploadResponse) -> Metadata {
        let mut meta = if path.ends_with('/') {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::file(resp.content_length)
        };

        if let Some(md5) = resp.content_md5 {
            meta.content_md5(&md5);
        }

        if let Some(content_type) = resp.content_type {
            meta.content_type(&content_type);
        }

        meta.build()
    }
}

impl oio::MultipartWrite for B2Writer {
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        let resp = self
            .core
            .upload_file(&self.ctx, &self.path, Some(size), &self.op, body)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let result: UploadResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                let meta = Self::parse_body_into_meta(&self.path, result);

                Ok(meta)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadFile")),
                resp,
            )),
        }
    }

    async fn initiate_part(&self) -> Result<String> {
        let resp = self
            .core
            .start_large_file(&self.ctx, &self.path, &self.op)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let result: StartLargeFileResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                Ok(result.file_id)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("StartLargeFile")),
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
        // B2 requires part number must between [1..=10000]
        let part_number = part_number + 1;

        let resp = self
            .core
            .upload_part(&self.ctx, upload_id, part_number, size, body)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let result: UploadPartResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                Ok(oio::MultipartPart {
                    etag: result.content_sha1,
                    part_number,
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
        let part_sha1_array = parts
            .iter()
            .map(|p| {
                let binding = p.etag.clone();
                let sha1 = binding.strip_prefix("unverified:");
                let Some(sha1) = sha1 else {
                    return "".to_string();
                };
                sha1.to_string()
            })
            .collect();

        let resp = self
            .core
            .finish_large_file(&self.ctx, upload_id, part_sha1_array)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let result: UploadResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                let meta = Self::parse_body_into_meta(&self.path, result);

                Ok(meta)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("FinishLargeFile")),
                resp,
            )),
        }
    }

    async fn abort_part(&self, upload_id: &str) -> Result<()> {
        let resp = self.core.cancel_large_file(&self.ctx, upload_id).await?;
        match resp.status() {
            // b2 returns code 200 if abort succeeds.
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CancelLargeFile")),
                resp,
            )),
        }
    }
}
