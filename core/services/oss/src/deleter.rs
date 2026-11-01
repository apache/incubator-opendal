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

use std::collections::HashSet;
use std::sync::Arc;

use bytes::Buf;
use http::StatusCode;

use super::core::parse_error;
use super::core::*;
use opendal_core::raw::oio::BatchDeleteResult;
use opendal_core::raw::*;
use opendal_core::*;

pub struct OssDeleter {
    core: Arc<OssCore>,
    ctx: OperationContext,
}

impl OssDeleter {
    pub fn new(core: Arc<OssCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::BatchDelete for OssDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        let resp = self.core.oss_delete_object(&self.ctx, &path, &args).await?;

        let status = resp.status();

        match status {
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteObject")),
                resp,
            )),
        }
    }

    async fn delete_batch(&self, batch: Vec<(String, OpDelete)>) -> Result<BatchDeleteResult> {
        // Sadly, OSS will not return failed keys, so we will build
        // a set to calculate the failed keys.
        let mut keys: HashSet<(String, OpDelete)> = batch
            .iter()
            .map(|path| (path.0.to_owned(), path.1.clone()))
            .collect();

        let resp = self.core.oss_delete_objects(&self.ctx, batch).await?;

        let status = resp.status();

        if status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteMultipleObjects")),
                resp,
            ));
        }

        let bs = resp.into_body();

        let result: DeleteObjectsResult =
            quick_xml::de::from_reader(bs.reader()).map_err(new_xml_deserialize_error)?;

        if result.deleted.is_empty() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "oss delete this key failed for reason we don't know",
            ));
        }

        let mut batched_result = BatchDeleteResult {
            succeeded: Vec::with_capacity(result.deleted.len()),
            // `result.deleted.len()` is server-controlled and may exceed `keys.len()`; use
            // `saturating_sub` (mirroring services/tos) to avoid an unsigned underflow that
            // wraps to a huge `with_capacity` (release: abort).
            failed: Vec::with_capacity(keys.len().saturating_sub(result.deleted.len())),
        };

        for i in result.deleted {
            let path = build_rel_path(&self.core.root, &i.key);
            let op = OpDelete::from_options(
                &Capability::default(),
                options::DeleteOptions {
                    version: i.version_id,
                    ..Default::default()
                },
            )
            .expect("delete result does not contain a logical condition");
            let object = (path, op);
            keys.remove(&object);
            batched_result.succeeded.push(object);
        }
        // TODO: we should handle those errors with code.
        for (path, op) in keys {
            batched_result.failed.push((
                path,
                op,
                Error::new(
                    ErrorKind::Unexpected,
                    "oss delete this key failed for reason we don't know",
                ),
            ));
        }

        Ok(batched_result)
    }
}
