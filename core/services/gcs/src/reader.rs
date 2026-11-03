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

use super::backend::*;
use super::core::ErrorContext;
use super::core::constants::X_GOOG_GENERATION;
use super::core::parse_error;
use http::Response;
use http::StatusCode;
use opendal_core::raw::*;
use opendal_core::*;

/// Reader returned by this backend.
pub struct GcsReader {
    backend: GcsBackend,
    // StreamReader opens requests lazily, so keep the per-operation context
    // available for later range requests.
    ctx: OperationContext,
    path: String,
    args: OpRead,
}

impl GcsReader {
    pub(super) fn new(
        backend: GcsBackend,
        ctx: OperationContext,
        path: &str,
        args: OpRead,
    ) -> Self {
        Self {
            backend,
            ctx,
            path: path.to_string(),
            args,
        }
    }
}

impl oio::StreamRead for GcsReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let args = self.args.clone();
        let error_ctx = ErrorContext::new(ServiceOperation("GetObject"))
            .with_caller_condition(args.is_conditional());
        let resp = backend
            .core
            .gcs_get_object(&self.ctx, path, range, &args)
            .await?;

        let status = resp.status();

        let (rp, stream) = match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                let mut metadata = parse_into_metadata(path, resp.headers())?.into_builder();
                if let Some(generation) = parse_header_to_str(resp.headers(), X_GOOG_GENERATION)? {
                    metadata.version(generation);
                }
                (RpRead::new(metadata.build()), resp.into_body())
            }
            _ => {
                let (part, mut body) = resp.into_parts();
                let buf = body.to_buffer().await?;
                return Err(parse_error(error_ctx, Response::from_parts(part, buf)));
            }
        };

        Ok((rp, Box::new(stream) as Box<dyn oio::ReadStreamDyn>))
    }
}
