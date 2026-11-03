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

use super::core::SloManifestEntry;
use super::core::parse_error;
use super::core::{ErrorContext, SwiftCore};
use opendal_core::raw::*;
use opendal_core::*;

pub struct SwiftWriter {
    core: Arc<SwiftCore>,
    ctx: OperationContext,
    op: OpWrite,
    path: String,
}

impl SwiftWriter {
    pub fn new(core: Arc<SwiftCore>, ctx: OperationContext, op: OpWrite, path: String) -> Self {
        SwiftWriter {
            core,
            ctx,
            op,
            path,
        }
    }

    fn parse_metadata(headers: &http::HeaderMap) -> Result<Metadata> {
        let mut metadata = MetadataBuilder::unknown();

        if let Some(etag) = parse_etag(headers)? {
            metadata.etag(etag);
        }

        if let Some(last_modified) = parse_last_modified(headers)? {
            metadata.last_modified(last_modified);
        }

        Ok(metadata.build())
    }
}

impl oio::MultipartWrite for SwiftWriter {
    async fn write_once(&self, _size: u64, bs: Buffer) -> Result<Metadata> {
        let resp = self
            .core
            .swift_create_object(&self.ctx, &self.path, bs.len() as u64, &self.op, bs)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => {
                let metadata = SwiftWriter::parse_metadata(resp.headers())?;
                Ok(metadata)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateObject")),
                resp,
            )),
        }
    }

    async fn initiate_part(&self) -> Result<String> {
        // Swift SLO doesn't need a server-side initiate call.
        // Generate a local UUID as the upload ID to namespace the segments.
        Ok(uuid::Uuid::new_v4().to_string())
    }

    async fn write_part(
        &self,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<oio::MultipartPart> {
        let resp = self
            .core
            .swift_put_segment(&self.ctx, &self.path, upload_id, part_number, size, body)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => {
                let etag = parse_etag(resp.headers())?
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::Unexpected,
                            "ETag not present in segment upload response",
                        )
                    })?
                    .to_string();

                Ok(oio::MultipartPart {
                    part_number,
                    etag,
                    checksum: None,
                    size: Some(size),
                })
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadSegment")),
                resp,
            )),
        }
    }

    async fn complete_part(
        &self,
        upload_id: &str,
        parts: &[oio::MultipartPart],
    ) -> Result<Metadata> {
        let manifest: Vec<SloManifestEntry> = parts
            .iter()
            .map(|part| {
                let segment = self
                    .core
                    .slo_segment_path(&self.path, upload_id, part.part_number);
                SloManifestEntry {
                    path: format!("{}/{}", self.core.container, segment),
                    etag: part.etag.trim_matches('"').to_string(),
                    size_bytes: part.size.unwrap_or(0),
                }
            })
            .collect();

        let resp = self
            .core
            .swift_put_slo_manifest(&self.ctx, &self.path, &manifest, &self.op)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => {
                let metadata = SwiftWriter::parse_metadata(resp.headers())?;
                Ok(metadata)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadManifest")),
                resp,
            )),
        }
    }

    async fn abort_part(&self, upload_id: &str) -> Result<()> {
        self.core
            .swift_delete_slo(&self.ctx, &self.path, upload_id)
            .await
    }
}
