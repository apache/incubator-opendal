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

use super::core::ErrorContext;
use super::core::GcsCore;
use super::core::RewriteResponse;
use super::core::constants::GCS_REWRITE_MAX_CHUNK_SIZE;
use super::core::constants::GCS_REWRITE_MIN_CHUNK_SIZE;
use super::core::parse_error;
use opendal_core::raw::*;
use opendal_core::*;

pub struct GcsCopier {
    core: Arc<GcsCore>,
    ctx: OperationContext,
    from: String,
    to: String,
    args: OpCopy,

    chunk: Option<usize>,
    rewrite_token: Option<String>,
    total_bytes_rewritten: u64,
    completed: bool,
    metadata: Option<Metadata>,
}

impl GcsCopier {
    pub fn new(
        core: Arc<GcsCore>,
        ctx: OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Self {
        let chunk = args.chunk().map(|v| {
            let v = v.clamp(GCS_REWRITE_MIN_CHUNK_SIZE, GCS_REWRITE_MAX_CHUNK_SIZE);
            v / GCS_REWRITE_MIN_CHUNK_SIZE * GCS_REWRITE_MIN_CHUNK_SIZE
        });

        Self {
            core,
            ctx,
            from: from.to_string(),
            to: to.to_string(),
            args,
            chunk,
            rewrite_token: None,
            total_bytes_rewritten: 0,
            completed: false,
            metadata: None,
        }
    }
}

impl oio::Copy for GcsCopier {
    async fn next(&mut self) -> Result<Option<usize>> {
        if self.completed {
            return Ok(None);
        }

        let resp = self
            .core
            .gcs_rewrite_object(
                &self.ctx,
                &self.from,
                &self.to,
                &self.args,
                self.chunk,
                self.rewrite_token.as_deref(),
            )
            .await?;

        if !resp.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("RewriteObject"))
                    .with_caller_condition(self.args.is_conditional()),
                resp,
            ));
        }

        let result: RewriteResponse = serde_json::from_reader(resp.into_body().reader())
            .map_err(new_json_deserialize_error)?;

        let total = result.total_bytes_rewritten.parse::<u64>().map_err(|err| {
            Error::new(
                ErrorKind::Unexpected,
                "invalid totalBytesRewritten in rewrite response",
            )
            .with_context("totalBytesRewritten", &result.total_bytes_rewritten)
            .set_source(err)
        })?;

        let progress = total
            .checked_sub(self.total_bytes_rewritten)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "rewrite response totalBytesRewritten moved backwards",
                )
                .with_context("previous", self.total_bytes_rewritten)
                .with_context("current", total)
            })?;

        self.total_bytes_rewritten = total;

        if result.done {
            self.completed = true;
            self.rewrite_token = None;
            self.metadata = Some(result.into_metadata(&self.to)?);
        } else {
            let token = result.rewrite_token.ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "rewrite response is not done but rewriteToken is missing",
                )
            })?;
            self.rewrite_token = Some(token);
        }

        if progress == 0 {
            return if self.completed {
                Ok(None)
            } else {
                Err(
                    Error::new(ErrorKind::Unexpected, "rewrite response made no progress")
                        .set_temporary(),
                )
            };
        }

        let progress = progress
            .try_into()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "rewrite progress exceeds usize"))?;
        Ok(Some(progress))
    }

    async fn close(&mut self) -> Result<Metadata> {
        while !self.completed {
            self.next().await?;
        }

        let metadata = self
            .metadata
            .clone()
            .unwrap_or_else(|| MetadataBuilder::unknown().build());
        let mut builder = metadata.into_builder();
        builder.set_file(self.total_bytes_rewritten);
        Ok(builder.build())
    }

    async fn abort(&mut self) -> Result<()> {
        self.rewrite_token = None;
        self.completed = true;
        self.metadata = None;
        Ok(())
    }
}
