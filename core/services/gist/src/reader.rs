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

use http::Response;
use http::StatusCode;

use super::backend::gist_file_name;
use super::core::ErrorContext;
use super::core::GistCore;
use super::core::parse_error;
use super::core::parse_metadata;
use opendal_core::raw::*;
use opendal_core::*;

/// Reader returned by this backend.
pub struct GistReader {
    core: Arc<GistCore>,
    ctx: OperationContext,
    path: String,
}

impl GistReader {
    pub(super) fn new(core: Arc<GistCore>, ctx: OperationContext, path: String) -> Self {
        Self { core, ctx, path }
    }

    fn not_found(&self, name: &str) -> Error {
        Error::new(ErrorKind::NotFound, "gist file does not exist").with_context("path", name)
    }
}

impl oio::StreamRead for GistReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let name = gist_file_name(&self.path)?;

        let mut file = match self.core.get_file(&self.ctx, name, Operation::Read).await? {
            Some(file) => file,
            None => return Err(self.not_found(name)),
        };

        let content = if file.truncated {
            None
        } else {
            file.content.take()
        };

        // Read metadata describes the whole file, not the requested range, so it
        // comes from the gist response rather than from the transfer headers.
        let metadata = parse_metadata(&file);

        // The API embeds content up to 1 MiB per file. Larger files come back
        // truncated and must be read from `raw_url` instead.
        if let Some(content) = content {
            let bs = Buffer::from(content);

            if range.is_full() {
                return Ok((
                    RpRead::new(metadata),
                    Box::new(bs) as Box<dyn oio::ReadStreamDyn>,
                ));
            }

            let sliced = bs.slice(range.to_content_range(bs.len())?);

            return Ok((
                RpRead::new(metadata),
                Box::new(sliced) as Box<dyn oio::ReadStreamDyn>,
            ));
        }

        let resp = self.core.fetch_raw(&self.ctx, &file.raw_url, range).await?;

        let status = resp.status();

        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                let (_, mut body) = resp.into_parts();

                // The raw host may answer a ranged request with the whole file,
                // so slice the body client-side when it did not honor the range.
                if status == StatusCode::PARTIAL_CONTENT || range.is_full() {
                    Ok((
                        RpRead::new(metadata),
                        Box::new(body) as Box<dyn oio::ReadStreamDyn>,
                    ))
                } else {
                    let bs = body.to_buffer().await?;
                    let sliced = bs.slice(range.to_content_range(bs.len())?);

                    Ok((
                        RpRead::new(metadata),
                        Box::new(sliced) as Box<dyn oio::ReadStreamDyn>,
                    ))
                }
            }
            _ => {
                let (part, mut body) = resp.into_parts();
                let bs = body.to_buffer().await?;

                Err(parse_error(
                    ErrorContext::new(ServiceOperation("GetGistRawFile")),
                    Response::from_parts(part, bs),
                ))
            }
        }
    }
}
