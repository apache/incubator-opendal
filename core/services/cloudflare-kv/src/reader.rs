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
use super::core::{ErrorContext, parse_error};
use super::model::*;
use bytes::Buf;
use http::StatusCode;
use opendal_core::raw::*;
use opendal_core::*;

/// Reader returned by this backend.
pub struct CloudflareKvReader {
    backend: CloudflareKvBackend,
    ctx: OperationContext,
    path: String,
    args: OpRead,
}

impl CloudflareKvReader {
    pub(super) fn new(
        backend: CloudflareKvBackend,
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

impl oio::StreamRead for CloudflareKvReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let args = self.args.clone();
        let path = build_abs_path(&backend.core.info.root(), path);
        let resp = backend.core.get(&self.ctx, &path).await?;

        let status = resp.status();

        if status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetValue")),
                resp,
            ));
        }

        let resp_body = resp.into_body();

        if args.is_conditional() {
            let meta_resp = backend.core.metadata(&self.ctx, &path).await?;

            if meta_resp.status() != StatusCode::OK {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("GetMetadata")),
                    meta_resp,
                ));
            }

            let cf_response: CfKvStatResponse =
                serde_json::from_reader(meta_resp.into_body().reader())
                    .map_err(new_json_deserialize_error)?;

            if !cf_response.success && cf_response.result.is_some() {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "cloudflare_kv read this key failed for reason we don't know",
                ));
            }

            let metadata = cf_response.result.unwrap();

            // Check if_match condition
            if let Some(if_match) = &args.if_match()
                && if_match != &metadata.etag
            {
                return Err(Error::new(ErrorKind::ConditionNotMatch, "etag mismatch"));
            }

            // Check if_none_match condition
            if let Some(if_none_match) = &args.if_none_match()
                && if_none_match == &metadata.etag
            {
                return Err(Error::new(
                    ErrorKind::ConditionNotMatch,
                    "etag match when expected none match",
                ));
            }

            // Parse since time once for both time-based conditions
            let last_modified = metadata
                .last_modified
                .parse::<Timestamp>()
                .map_err(|_| Error::new(ErrorKind::Unsupported, "invalid since format"))?;

            // Check modified_since condition
            if let Some(modified_since) = &args.if_modified_since()
                && !last_modified.gt(modified_since)
            {
                return Err(Error::new(
                    ErrorKind::ConditionNotMatch,
                    "not modified since specified time",
                ));
            }

            // Check unmodified_since condition
            if let Some(unmodified_since) = &args.if_unmodified_since()
                && !last_modified.le(unmodified_since)
            {
                return Err(Error::new(
                    ErrorKind::ConditionNotMatch,
                    "modified since specified time",
                ));
            }
        }

        let total_size = resp_body.len() as u64;
        let buffer = resp_body.slice(range.to_content_range(resp_body.len())?);
        let metadata = {
            let metadata = MetadataBuilder::file(total_size);
            metadata.build()
        };
        Ok((
            RpRead::new(metadata),
            Box::new(buffer) as Box<dyn oio::ReadStreamDyn>,
        ))
    }
}
