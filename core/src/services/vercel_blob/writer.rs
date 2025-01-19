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

use super::core::InitiateMultipartUploadResponse;
use super::core::Part;
use super::core::UploadPartResponse;
use super::core::VercelBlobCore;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub type VercelBlobWriters = oio::MultipartWriter<VercelBlobWriter>;

pub struct VercelBlobWriter {
    core: Arc<VercelBlobCore>,
    op: OpWrite,
    path: String,
}

impl VercelBlobWriter {
    pub fn new(core: Arc<VercelBlobCore>, op: OpWrite, path: String) -> Self {
        VercelBlobWriter { core, op, path }
    }
}

impl oio::MultipartWrite for VercelBlobWriter {
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        let req = self
            .core
            .get_put_request(&self.path, Some(size), &self.op, body)?;

        let resp = self.core.send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn initiate_part(&self) -> Result<String> {
        let resp = self
            .core
            .initiate_multipart_upload(&self.path, &self.op)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let resp: InitiateMultipartUploadResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                Ok(resp.upload_id)
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
    ) -> Result<oio::MultipartPart> {
        let part_number = part_number + 1;

        let resp = self
            .core
            .upload_part(&self.path, upload_id, part_number, size, body)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let resp: UploadPartResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                Ok(oio::MultipartPart {
                    part_number,
                    etag: resp.etag,
                    checksum: None,
                })
            }
            _ => Err(parse_error(resp)),
        }
    }

    async fn complete_part(
        &self,
        upload_id: &str,
        parts: &[oio::MultipartPart],
    ) -> Result<Metadata> {
        let parts = parts
            .iter()
            .map(|p| Part {
                part_number: p.part_number,
                etag: p.etag.clone(),
            })
            .collect::<Vec<Part>>();

        let resp = self
            .core
            .complete_multipart_upload(&self.path, upload_id, parts)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn abort_part(&self, _upload_id: &str) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "VercelBlob does not support abort multipart upload",
        ))
    }
}
