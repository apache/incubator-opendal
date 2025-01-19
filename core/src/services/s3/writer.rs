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

use super::core::*;
use super::error::from_s3_error;
use super::error::parse_error;
use super::error::S3Error;
use crate::raw::*;
use crate::*;
use bytes::Buf;
use constants::{X_AMZ_OBJECT_SIZE, X_AMZ_VERSION_ID};
use http::StatusCode;

pub type S3Writers = oio::MultipartWriter<S3Writer>;

pub struct S3Writer {
    core: Arc<S3Core>,

    op: OpWrite,
    path: String,
}

impl S3Writer {
    pub fn new(core: Arc<S3Core>, path: &str, op: OpWrite) -> Self {
        S3Writer {
            core,
            path: path.to_string(),
            op,
        }
    }

    fn parse_header_into_meta(path: &str, headers: &http::HeaderMap) -> Result<Metadata> {
        let mut meta = Metadata::new(EntryMode::from_path(path));
        if let Some(etag) = parse_etag(headers)? {
            meta.set_etag(etag);
        }
        if let Some(version) = parse_header_to_str(headers, X_AMZ_VERSION_ID)? {
            meta.set_version(version);
        }
        if let Some(size) = parse_header_to_str(headers, X_AMZ_OBJECT_SIZE)? {
            if let Ok(value) = size.parse() {
                meta.set_content_length(value);
            }
        }
        Ok(meta)
    }
}

impl oio::MultipartWrite for S3Writer {
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        let mut req = self
            .core
            .s3_put_object_request(&self.path, Some(size), &self.op, body)?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();

        let meta = S3Writer::parse_header_into_meta(&self.path, resp.headers())?;

        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(meta),
            _ => Err(parse_error(resp)),
        }
    }

    async fn initiate_part(&self) -> Result<String> {
        let resp = self
            .core
            .s3_initiate_multipart_upload(&self.path, &self.op)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let result: InitiateMultipartUploadResult =
                    quick_xml::de::from_reader(bs.reader()).map_err(new_xml_deserialize_error)?;

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
    ) -> Result<oio::MultipartPart> {
        // AWS S3 requires part number must between [1..=10000]
        let part_number = part_number + 1;

        let checksum = self.core.calculate_checksum(&body);

        let mut req = self.core.s3_upload_part_request(
            &self.path,
            upload_id,
            part_number,
            size,
            body,
            checksum.clone(),
        )?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

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
                    checksum,
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
            .map(|p| match &self.core.checksum_algorithm {
                None => CompleteMultipartUploadRequestPart {
                    part_number: p.part_number,
                    etag: p.etag.clone(),
                    ..Default::default()
                },
                Some(checksum_algorithm) => match checksum_algorithm {
                    ChecksumAlgorithm::Crc32c => CompleteMultipartUploadRequestPart {
                        part_number: p.part_number,
                        etag: p.etag.clone(),
                        checksum_crc32c: p.checksum.clone(),
                    },
                },
            })
            .collect();

        let resp = self
            .core
            .s3_complete_multipart_upload(&self.path, upload_id, parts)
            .await?;

        let status = resp.status();

        let mut meta = S3Writer::parse_header_into_meta(&self.path, resp.headers())?;

        match status {
            StatusCode::OK => {
                // still check if there is any error because S3 might return error for status code 200
                // https://docs.aws.amazon.com/AmazonS3/latest/API/API_CompleteMultipartUpload.html#API_CompleteMultipartUpload_Example_4
                let (parts, body) = resp.into_parts();

                let maybe_error: S3Error = quick_xml::de::from_reader(body.clone().reader())
                    .map_err(new_xml_deserialize_error)?;
                if !maybe_error.code.is_empty() {
                    return Err(from_s3_error(maybe_error, parts));
                }

                let ret: CompleteMultipartUploadResult =
                    quick_xml::de::from_reader(body.reader()).map_err(new_xml_deserialize_error)?;
                meta.set_etag(&ret.etag);

                Ok(meta)
            }
            _ => Err(parse_error(resp)),
        }
    }

    async fn abort_part(&self, upload_id: &str) -> Result<()> {
        let resp = self
            .core
            .s3_abort_multipart_upload(&self.path, upload_id)
            .await?;
        match resp.status() {
            // s3 returns code 204 if abort succeeds.
            StatusCode::NO_CONTENT => Ok(()),
            _ => Err(parse_error(resp)),
        }
    }
}
