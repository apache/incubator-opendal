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

use super::core::parse_error;
use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;

/// Write progress of the streaming writer.
///
/// The writer starts in [`Progress::New`] and moves to [`Progress::Started`]
/// once the first chunk has been written.
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
enum Progress {
    /// No chunk has been written yet: the next chunk creates or truncates the file.
    New,
    /// The file already holds `offset` bytes: the next chunk appends with `Content-Range`.
    Started {
        /// Offset of the next chunk, equal to the number of bytes written so far.
        offset: u64,
        /// Metadata parsed from the latest write response.
        metadata: Metadata,
    },
}

/// WebDAV writer that streams data in ordered chunks.
///
/// The first chunk uses a regular PUT, which creates or truncates the file.
/// Following chunks use partial PUTs carrying `Content-Range`, so the writer
/// never needs to hold the whole file in memory.
pub struct WebdavWriter {
    core: Arc<WebdavCore>,
    ctx: OperationContext,

    op: OpWrite,
    path: String,

    /// Write progress that decides how the next chunk is sent.
    progress: Progress,
}

impl WebdavWriter {
    pub fn new(core: Arc<WebdavCore>, ctx: OperationContext, op: OpWrite, path: String) -> Self {
        WebdavWriter {
            core,
            ctx,
            op,
            path,
            progress: Progress::New,
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

    /// Parse a write response, returning its metadata on success.
    fn parse_write_response(resp: http::Response<Buffer>) -> Result<Metadata> {
        match resp.status() {
            StatusCode::CREATED | StatusCode::OK | StatusCode::NO_CONTENT => {
                Self::parse_metadata(resp.headers())
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Put")),
                resp,
            )),
        }
    }

    /// Write the first chunk with a regular PUT, which creates or truncates the file.
    async fn write_first(&mut self, bs: Buffer) -> Result<Metadata> {
        // Ensure parent path exists unless disabled for servers that don't support PROPFIND.
        if !self.core.disable_create_dir {
            self.core
                .webdav_mkcol(&self.ctx, get_parent(&self.path))
                .await?;
        }

        let resp = self
            .core
            .webdav_put(&self.ctx, &self.path, Some(bs.len() as u64), &self.op, bs)
            .await?;
        let metadata = Self::parse_write_response(resp)?;

        // Set user metadata using PROPPATCH if provided
        if let Some(user_metadata) = self.op.user_metadata() {
            let user_metadata = user_metadata
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect();
            let proppatch_resp = self
                .core
                .webdav_proppatch(&self.ctx, &self.path, &user_metadata)
                .await?;

            let proppatch_status = proppatch_resp.status();
            // PROPPATCH returns 207 Multi-Status - need to check response body
            // for actual success/failure status
            if proppatch_status == StatusCode::MULTI_STATUS {
                let body = proppatch_resp.into_body().to_bytes();
                let xml = String::from_utf8_lossy(&body);
                check_proppatch_response(&xml)?;
            } else if !proppatch_status.is_success() {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("Proppatch")),
                    proppatch_resp,
                ));
            }
        }

        Ok(metadata)
    }

    /// Write a following chunk at `offset` with a partial PUT carrying `Content-Range`.
    async fn write_next(&mut self, bs: Buffer, offset: u64) -> Result<Metadata> {
        let resp = self
            .core
            .webdav_put_range(&self.ctx, &self.path, offset, &self.op, bs)
            .await?;
        let metadata = Self::parse_write_response(resp)?;
        Ok(metadata)
    }
}

impl oio::Write for WebdavWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let size = bs.len();
        if size == 0 {
            return Ok(());
        }

        let offset = match &self.progress {
            Progress::New => None,
            Progress::Started { offset, .. } => Some(*offset),
        };
        match offset {
            None => {
                let metadata = self.write_first(bs).await?;
                self.progress = Progress::Started {
                    offset: size as u64,
                    metadata,
                };
            }
            Some(offset) => {
                let metadata = self.write_next(bs, offset).await?;
                self.progress = Progress::Started {
                    offset: offset + size as u64,
                    metadata,
                };
            }
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        if matches!(self.progress, Progress::New) {
            // Keep the semantics of the one-shot writer: create an empty file
            // when nothing has been written.
            let metadata = self.write_first(Buffer::new()).await?;
            self.progress = Progress::Started {
                offset: 0,
                metadata,
            };
        }

        match &self.progress {
            Progress::Started { metadata, .. } => Ok(metadata.clone()),
            // `Progress::New` moves to `Progress::Started` above, so this branch
            // is unreachable; return unknown metadata instead of panicking.
            Progress::New => Ok(MetadataBuilder::unknown().build()),
        }
    }

    async fn abort(&mut self) -> Result<()> {
        if matches!(self.progress, Progress::New) {
            return Ok(());
        }

        let resp = self.core.webdav_delete(&self.ctx, &self.path).await?;
        match resp.status() {
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Delete")),
                resp,
            )),
        }
    }
}
