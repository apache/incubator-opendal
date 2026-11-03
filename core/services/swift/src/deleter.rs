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

pub struct SwiftDeleter {
    core: Arc<SwiftCore>,
    ctx: OperationContext,
}

impl SwiftDeleter {
    pub fn new(core: Arc<SwiftCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::BatchDelete for SwiftDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let resp = self.core.swift_delete(&self.ctx, &path).await?;

        let status = resp.status();

        match status {
            StatusCode::NO_CONTENT | StatusCode::OK => Ok(()),
            StatusCode::NOT_FOUND => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteObject")),
                resp,
            )),
        }
    }

    async fn delete_batch(&self, batch: Vec<(String, OpDelete)>) -> Result<oio::BatchDeleteResult> {
        let resp = self.core.swift_bulk_delete(&self.ctx, &batch).await?;

        let status = resp.status();
        if status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("BulkDelete")),
                resp,
            ));
        }

        let bs = resp.into_body().to_bytes();
        let result: BulkDeleteResponse =
            serde_json::from_slice(&bs).map_err(new_json_deserialize_error)?;

        let mut batched_result = oio::BatchDeleteResult {
            // `result.errors.len()` is server-controlled and may exceed `batch.len()`; use
            // `saturating_sub` (mirroring services/tos) to avoid an unsigned underflow that
            // wraps to a huge `with_capacity` (release: abort).
            succeeded: Vec::with_capacity(batch.len().saturating_sub(result.errors.len())),
            failed: Vec::with_capacity(result.errors.len()),
        };

        for (path, op) in batch {
            // Check if this path appears in the errors list.
            // The error paths from Swift include the container prefix, so we need
            // to reconstruct the full path for comparison.
            let abs = build_abs_path(&self.core.root, &path);
            let full_path = format!("{}/{}", self.core.container, abs);

            if let Some(error_entry) = result.errors.iter().find(|e| {
                e.first()
                    .map(|p| percent_decode_path(p) == full_path)
                    .unwrap_or(false)
            }) {
                let status_str = error_entry.get(1).cloned().unwrap_or_default();
                batched_result.failed.push((
                    path,
                    op,
                    Error::new(
                        ErrorKind::Unexpected,
                        format!("bulk delete error: {status_str}"),
                    ),
                ));
            } else {
                // Either deleted successfully or not found (both are success for us).
                batched_result.succeeded.push((path, op));
            }
        }

        Ok(batched_result)
    }
}
