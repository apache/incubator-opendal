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
use opendal_core::raw::oio::BatchDeleteResult;
use opendal_core::raw::*;
use opendal_core::*;

pub struct AzblobDeleter {
    core: Arc<AzblobCore>,
    ctx: OperationContext,
}

impl AzblobDeleter {
    pub fn new(core: Arc<AzblobCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::BatchDelete for AzblobDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        let resp = self
            .core
            .azblob_delete_blob(&self.ctx, &path, &args)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::ACCEPTED => Ok(()),
            StatusCode::NOT_FOUND if args.if_match().is_some() => Err(Error::new(
                ErrorKind::ConditionNotMatch,
                "delete precondition requires a live target",
            )),
            StatusCode::NOT_FOUND => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteBlob"))
                    .with_caller_condition(args.is_conditional()),
                resp,
            )),
        }
    }

    async fn delete_batch(&self, batch: Vec<(String, OpDelete)>) -> Result<BatchDeleteResult> {
        // TODO: Add remove version support.
        // construct and complete batch request
        let resp = self.core.azblob_batch_delete(&self.ctx, &batch).await?;

        // check response status
        if resp.status() != StatusCode::ACCEPTED {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("BatchDeleteBlobs")),
                resp,
            ));
        }

        // get boundary from response header
        let boundary = parse_multipart_boundary(resp.headers())?
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "invalid response: no boundary provided in header",
                )
            })?
            .to_string();

        let bs = resp.into_body().to_bytes();
        let multipart: Multipart<MixedPart> =
            Multipart::new().with_boundary(&boundary).parse(bs)?;
        let parts = multipart.into_parts();

        if batch.len() != parts.len() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "invalid batch response, requests and response parts don't match",
            ));
        }

        let mut batched_result = BatchDeleteResult::default();

        for (part, (path, args)) in parts.into_iter().zip(batch) {
            let resp = part.into_response();

            if resp.status() == StatusCode::NOT_FOUND && args.if_match().is_some() {
                batched_result.failed.push((
                    path,
                    args,
                    Error::new(
                        ErrorKind::ConditionNotMatch,
                        "delete precondition requires a live target",
                    ),
                ));
            } else if resp.status() == StatusCode::ACCEPTED
                || resp.status() == StatusCode::NOT_FOUND
            {
                batched_result.succeeded.push((path, args));
            } else {
                let error_ctx = ErrorContext::new(ServiceOperation("BatchDeleteBlobs"))
                    .with_caller_condition(args.is_conditional());
                let err = parse_error(error_ctx, resp);
                batched_result.failed.push((path, args, err));
            }
        }

        // If no object is deleted, return directly.
        if batched_result.succeeded.is_empty() {
            let err = batched_result.failed.remove(0).2;
            return Err(err);
        }

        Ok(batched_result)
    }
}
