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

use http::StatusCode;

use super::core::*;
use super::error::parse_error;
use crate::raw::oio::MultipartPart;
use crate::raw::*;
use crate::*;

pub type ObsWriters = TwoWays<oio::MultipartWriter<ObsWriter>, oio::AppendWriter<ObsWriter>>;

pub struct ObsWriter {
    core: Arc<ObsCore>,

    op: OpWrite,
    path: String,
}

impl ObsWriter {
    pub fn new(core: Arc<ObsCore>, path: &str, op: OpWrite) -> Self {
        ObsWriter {
            core,
            path: path.to_string(),
            op,
        }
    }
}

impl oio::MultipartWrite for ObsWriter {
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        let mut req = self
            .core
            .obs_put_object_request(&self.path, Some(size), &self.op, body)?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn initiate_part(&self) -> Result<String> {
        let resp = self
            .core
            .obs_initiate_multipart_upload(&self.path, self.op.content_type())
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
            _ => Err(parse_error(resp)),
        }
    }

    async fn write_part(
        &self,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<MultipartPart> {
        // Obs service requires part number must between [1..=10000]
        let part_number = part_number + 1;

        let resp = self
            .core
            .obs_upload_part_request(&self.path, upload_id, part_number, Some(size), body)
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

                Ok(MultipartPart {
                    part_number,
                    etag,
                    checksum: None,
                })
            }
            _ => Err(parse_error(resp)),
        }
    }

    async fn complete_part(&self, upload_id: &str, parts: &[MultipartPart]) -> Result<Metadata> {
        let parts = parts
            .iter()
            .map(|p| CompleteMultipartUploadRequestPart {
                part_number: p.part_number,
                etag: p.etag.clone(),
            })
            .collect();

        let resp = self
            .core
            .obs_complete_multipart_upload(&self.path, upload_id, parts)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn abort_part(&self, upload_id: &str) -> Result<()> {
        let resp = self
            .core
            .obs_abort_multipart_upload(&self.path, upload_id)
            .await?;
        match resp.status() {
            // Obs returns code 204 No Content if abort succeeds.
            // Reference: https://support.huaweicloud.com/intl/en-us/api-obs/obs_04_0103.html
            StatusCode::NO_CONTENT => Ok(()),
            _ => Err(parse_error(resp)),
        }
    }
}

impl oio::AppendWrite for ObsWriter {
    async fn offset(&self) -> Result<u64> {
        let resp = self
            .core
            .obs_head_object(&self.path, &OpStat::default())
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
            _ => Err(parse_error(resp)),
        }
    }

    async fn append(&self, offset: u64, size: u64, body: Buffer) -> Result<()> {
        let mut req = self
            .core
            .obs_append_object_request(&self.path, offset, size, &self.op, body)?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(resp)),
        }
    }
}
