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

use super::core::parse_error;
use super::core::*;
use super::model::CfKvDeleteResponse;

pub struct CloudflareKvDeleter {
    core: Arc<CloudflareKvCore>,
    ctx: OperationContext,
}

impl CloudflareKvDeleter {
    pub fn new(core: Arc<CloudflareKvCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::BatchDelete for CloudflareKvDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let path = build_abs_path(&self.core.info.root(), &path);
        let resp = self
            .core
            .delete(&self.ctx, &[path.trim_end_matches('/').to_string()])
            .await?;

        let status = resp.status();

        if status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("BulkDelete")),
                resp.clone(),
            ));
        }

        let bs = resp.clone().into_body();
        let res: CfKvDeleteResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        if !res.success {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("BulkDelete")),
                resp.clone(),
            ));
        }

        Ok(())
    }

    async fn delete_batch(&self, batch: Vec<(String, OpDelete)>) -> Result<BatchDeleteResult> {
        let keys = batch
            .iter()
            .map(|path| {
                let path = build_abs_path(&self.core.info.root(), &path.0);
                path.trim_end_matches('/').to_string()
            })
            .collect::<Vec<String>>();

        let resp = self.core.delete(&self.ctx, &keys).await?;

        let status = resp.status();

        if status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("BulkDelete")),
                resp,
            ));
        }

        let bs = resp.into_body();

        let res: CfKvDeleteResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        let result = match (res.success, res.result) {
            (true, Some(result)) => result,
            _ => {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "cloudflare_kv delete this key failed for reason we don't know",
                ));
            }
        };

        let mut batched_result = BatchDeleteResult {
            // `successful_key_count` is taken verbatim from the server response; cap it to
            // the number of keys we actually requested so a malicious/compromised endpoint
            // cannot drive `Vec::with_capacity` to an abort/OOM. (`unsuccessful_keys` is an
            // already-parsed Vec, so its length is inherently bounded.)
            succeeded: Vec::with_capacity(result.successful_key_count.min(batch.len())),
            failed: Vec::with_capacity(result.unsuccessful_keys.len()),
        };

        for item in batch {
            if result.unsuccessful_keys.contains(&item.0) {
                batched_result.failed.push((
                    item.0,
                    item.1,
                    Error::new(
                        ErrorKind::Unexpected,
                        "cloudflare_kv delete this key failed for reason we don't know",
                    ),
                ));
            } else {
                batched_result.succeeded.push(item);
            }
        }

        Ok(batched_result)
    }
}
