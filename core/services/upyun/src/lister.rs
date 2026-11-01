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
use opendal_core::raw::*;
use opendal_core::*;

use super::core::{ErrorContext, ListObjectsResponse, UpyunCore, parse_error};

pub struct UpyunLister {
    core: Arc<UpyunCore>,
    ctx: OperationContext,

    path: String,
    limit: Option<usize>,
}

impl UpyunLister {
    pub(super) fn new(
        core: Arc<UpyunCore>,
        ctx: OperationContext,
        path: &str,
        limit: Option<usize>,
    ) -> Self {
        UpyunLister {
            core,
            ctx,
            path: path.to_string(),
            limit,
        }
    }
}

impl oio::PageList for UpyunLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self
            .core
            .list_objects(&self.ctx, &self.path, &ctx.token, self.limit)
            .await?;

        if resp.status() == http::StatusCode::NOT_FOUND {
            ctx.done = true;
            return Ok(());
        }

        match resp.status() {
            http::StatusCode::OK => {}
            http::StatusCode::NOT_FOUND => {
                ctx.done = true;
                return Ok(());
            }
            _ => {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("ListObjects")),
                    resp,
                ));
            }
        }

        let bs = resp.into_body();

        let response: ListObjectsResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        // ref https://help.upyun.com/knowledge-base/rest_api/#e88eb7e58f96e79baee5bd95e69687e4bbb6e58897e8a1a8
        // when iter is "g2gCZAAEbmV4dGQAA2VvZg", it means the list is done.
        ctx.done = response.iter == "g2gCZAAEbmV4dGQAA2VvZg";

        ctx.token = response.iter;

        for file in response.files {
            let path = build_abs_path(&normalize_root(&self.path), &file.name);

            let entry = if file.type_field == "folder" {
                let path = format!("{path}/");
                oio::Entry::new(&path, MetadataBuilder::dir().build())
            } else {
                let mut m = MetadataBuilder::file(file.length);
                m.content_type(file.type_field)
                    .last_modified(Timestamp::from_second(file.last_modified)?);
                oio::Entry::new(&path, m.build())
            };

            ctx.entries.push_back(entry);
        }

        Ok(())
    }
}
