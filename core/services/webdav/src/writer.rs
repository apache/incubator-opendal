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

pub struct WebdavWriter {
    core: Arc<WebdavCore>,
    ctx: OperationContext,

    op: OpWrite,
    path: String,
}

impl WebdavWriter {
    pub fn new(core: Arc<WebdavCore>, ctx: OperationContext, op: OpWrite, path: String) -> Self {
        WebdavWriter {
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

impl oio::OneShotWrite for WebdavWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
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

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK | StatusCode::NO_CONTENT => {
                let metadata = WebdavWriter::parse_metadata(resp.headers())?;

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
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Put")),
                resp,
            )),
        }
    }
}
