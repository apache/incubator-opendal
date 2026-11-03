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
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;

use super::core::parse_error;
use super::core::*;
use opendal_core::Buffer;
use opendal_core::Error;
use opendal_core::ErrorKind;
use opendal_core::Metadata;
use opendal_core::MetadataBuilder;
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::*;

pub type CosWriters = TwoWays<oio::MultipartWriter<CosWriter>, oio::AppendWriter<CosWriter>>;

pub struct CosWriter {
    core: Arc<CosCore>,
    ctx: OperationContext,

    op: OpWrite,
    path: String,
}

impl CosWriter {
    pub fn new(core: Arc<CosCore>, ctx: OperationContext, path: &str, op: OpWrite) -> Self {
        CosWriter {
            core,
            ctx,
            path: path.to_string(),
            op,
        }
    }

    fn parse_metadata(headers: &HeaderMap<HeaderValue>) -> Result<Metadata> {
        let mut meta = MetadataBuilder::unknown();
        if let Some(etag) = parse_etag(headers)? {
            meta.etag(etag);
        }
        if let Some(md5) = parse_content_md5(headers)? {
            meta.content_md5(md5);
        }
        if let Some(version) = parse_header_to_str(headers, constants::X_COS_VERSION_ID)?
            && version != "null"
        {
            meta.version(version);
        }

        Ok(meta.build())
    }
}

impl oio::MultipartWrite for CosWriter {
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        let req = self
            .core
            .cos_put_object_request(&self.path, Some(size), &self.op, body)?;

        let req = self.core.sign(&self.ctx, req).await?;

        let resp = self.core.send(&self.ctx, req).await?;

        let meta = Self::parse_metadata(resp.headers())?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(meta),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("PutObject"))
                    .with_if_not_exists(self.op.if_not_exists()),
                resp,
            )),
        }
    }

    async fn initiate_part(&self) -> Result<String> {
        let resp = self
            .core
            .cos_initiate_multipart_upload(&self.ctx, &self.path, &self.op)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let result: InitiateMultipartUploadResult =
                    quick_xml::de::from_reader(bytes::Buf::reader(bs))
                        .map_err(new_xml_deserialize_error)?;

                Ok(result.upload_id)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("InitiateMultipartUpload"))
                    .with_if_not_exists(self.op.if_not_exists()),
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
        // COS requires part number must between [1..=10000]
        let part_number = part_number + 1;

        let resp = self
            .core
            .cos_upload_part_request(&self.ctx, &self.path, upload_id, part_number, size, body)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let etag = parse_etag(resp.headers())?
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

        let mut resp = self
            .core
            .cos_complete_multipart_upload(&self.ctx, &self.path, upload_id, parts, &self.op)
            .await?;

        let mut meta = Self::parse_metadata(resp.headers())?.into_builder();

        let status = resp.status();
        if status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CompleteMultipartUpload"))
                    .with_if_not_exists(self.op.if_not_exists()),
                resp,
            ));
        }

        let result: CompleteMultipartUploadResult =
            quick_xml::de::from_reader(resp.body_mut().reader())
                .map_err(new_xml_deserialize_error)?;
        meta.etag(&result.etag);
        Ok(meta.build())
    }

    async fn abort_part(&self, upload_id: &str) -> Result<()> {
        let resp = self
            .core
            .cos_abort_multipart_upload(&self.ctx, &self.path, upload_id)
            .await?;
        match resp.status() {
            // cos returns code 204 if abort succeeds.
            // Reference: https://www.tencentcloud.com/document/product/436/7740
            StatusCode::NO_CONTENT => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("AbortMultipartUpload")),
                resp,
            )),
        }
    }
}

impl oio::AppendWrite for CosWriter {
    async fn offset(&self) -> Result<u64> {
        let resp = self
            .core
            .cos_head_object(&self.ctx, &self.path, &OpStat::default())
            .await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let content_length = parse_content_length(resp.headers())?.ok_or_else(|| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "Content-Length not present in returning response",
                    )
                })?;
                Ok(content_length)
            }
            StatusCode::NOT_FOUND => Ok(0),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("HeadObject")),
                resp,
            )),
        }
    }

    async fn append(&self, offset: u64, size: u64, body: Buffer) -> Result<Metadata> {
        let req = self
            .core
            .cos_append_object_request(&self.path, offset, size, &self.op, body)?;

        let req = self.core.sign(&self.ctx, req).await?;

        let resp = self.core.send(&self.ctx, req).await?;

        let meta = Self::parse_metadata(resp.headers())?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(meta),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("AppendObject")),
                resp,
            )),
        }
    }
}
