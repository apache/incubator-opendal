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

use super::core::CompleteMultipartUploadRequestPart;
use super::core::GcsCore;
use super::core::InitiateMultipartUploadResult;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub type GcsWriters = oio::MultipartWriter<GcsWriter>;

pub struct GcsWriter {
    core: Arc<GcsCore>,
    path: String,
    op: OpWrite,
}

impl GcsWriter {
    pub fn new(core: Arc<GcsCore>, path: &str, op: OpWrite) -> Self {
        GcsWriter {
            core,
            path: path.to_string(),
            op,
        }
    }
}

impl oio::MultipartWrite for GcsWriter {
    async fn write_once(&self, _: u64, body: Buffer) -> Result<Metadata> {
        let size = body.len() as u64;
        let mut req = self.core.gcs_insert_object_request(
            &percent_encode_path(&self.path),
            Some(size),
            &self.op,
            body,
        )?;

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
            .gcs_initiate_multipart_upload(&percent_encode_path(&self.path))
            .await?;

        if !resp.status().is_success() {
            return Err(parse_error(resp));
        }

        let buf = resp.into_body();
        let upload_id: InitiateMultipartUploadResult =
            quick_xml::de::from_reader(buf.reader()).map_err(new_xml_deserialize_error)?;
        Ok(upload_id.upload_id)
    }

    async fn write_part(
        &self,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<oio::MultipartPart> {
        // Gcs requires part number must between [1..=10000]
        let part_number = part_number + 1;

        let resp = self
            .core
            .gcs_upload_part(&self.path, upload_id, part_number, size, body)
            .await?;

        if !resp.status().is_success() {
            return Err(parse_error(resp));
        }

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
        })
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
            .gcs_complete_multipart_upload(&self.path, upload_id, parts)
            .await?;

        if !resp.status().is_success() {
            return Err(parse_error(resp));
        }
        Ok(Metadata::default())
    }

    async fn abort_part(&self, upload_id: &str) -> Result<()> {
        let resp = self
            .core
            .gcs_abort_multipart_upload(&self.path, upload_id)
            .await?;
        match resp.status() {
            // gcs returns code 204 if abort succeeds.
            StatusCode::NO_CONTENT => Ok(()),
            _ => Err(parse_error(resp)),
        }
    }
}
