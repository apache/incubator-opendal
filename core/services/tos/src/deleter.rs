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
use http::StatusCode;
use opendal_core::raw::oio::BatchDeleteResult;
use opendal_core::raw::*;
use opendal_core::*;

use crate::core::parse_tos_error_code;
use crate::core::*;

pub struct TosDeleter {
    core: Arc<TosCore>,
    ctx: OperationContext,
}

impl TosDeleter {
    pub fn new(core: Arc<TosCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::BatchDelete for TosDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        if self.core.root == "/" && path == "/" {
            return Ok(());
        }

        let resp = self.core.tos_delete_object(&self.ctx, &path, &args).await?;

        let status = resp.status();

        match status {
            StatusCode::NO_CONTENT => Ok(()),
            StatusCode::NOT_FOUND => Ok(()),
            _ => Err(crate::core::parse_error(
                crate::core::ErrorContext::new(ServiceOperation("DeleteObject")),
                resp,
            )),
        }
    }

    async fn delete_batch(&self, batch: Vec<(String, OpDelete)>) -> Result<BatchDeleteResult> {
        let resp = self.core.tos_delete_objects(&self.ctx, &batch).await?;

        let status = resp.status();
        if status != StatusCode::OK {
            return Err(crate::core::parse_error(
                crate::core::ErrorContext::new(ServiceOperation("DeleteMultipleObjects")),
                resp,
            ));
        }

        let bs = resp.into_body();

        let result: DeleteObjectsResult =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        let mut deleted = result.deleted;
        let mut errors = result.errors;
        let mut batched_result = BatchDeleteResult {
            succeeded: Vec::with_capacity(batch.len().saturating_sub(errors.len())),
            failed: Vec::with_capacity(errors.len()),
        };
        for (path, op) in batch {
            let abs_path = build_abs_path(&self.core.root, &path);
            if let Some(idx) = errors
                .iter()
                .position(|e| e.key == abs_path && e.version_id.as_deref() == op.version())
            {
                let error = errors.swap_remove(idx);
                batched_result
                    .failed
                    .push((path, op, parse_delete_objects_result_error(error)));
            } else if let Some(idx) = deleted.iter().position(|e| {
                e.key == abs_path
                    && (op.version().is_none() || e.version_id.as_deref() == op.version())
            }) {
                let deleted = deleted.swap_remove(idx);
                let op = if op.version().is_some() {
                    if let Some(version) = &deleted.version_id {
                        op.into_version(version)
                    } else {
                        op
                    }
                } else {
                    op
                };
                batched_result.succeeded.push((path, op));
            } else {
                batched_result.succeeded.push((path, op));
            }
        }

        Ok(batched_result)
    }
}

fn parse_delete_objects_result_error(err: DeleteObjectsResultError) -> Error {
    let (kind, retryable) =
        parse_tos_error_code(err.code.as_str()).unwrap_or((ErrorKind::Unexpected, false));
    let mut err = Error::new(kind, format!("{err:?}"));
    if retryable {
        err = err.set_temporary();
    }
    err
}
