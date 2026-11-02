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

use super::core::ListResponse;
use super::core::parse_error;
use super::core::{ErrorContext, KoofrCore};
use opendal_core::MetadataBuilder;
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::oio::Entry;
use opendal_core::raw::*;

pub struct KoofrLister {
    core: Arc<KoofrCore>,
    ctx: OperationContext,

    path: String,
}

impl KoofrLister {
    pub(super) fn new(core: Arc<KoofrCore>, ctx: OperationContext, path: &str) -> Self {
        KoofrLister {
            core,
            ctx,
            path: path.to_string(),
        }
    }
}

impl oio::PageList for KoofrLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self.core.list(&self.ctx, &self.path).await?;

        if resp.status() == http::StatusCode::NOT_FOUND {
            ctx.done = true;
            return Ok(());
        }

        match resp.status() {
            http::StatusCode::OK => {}
            _ => {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("FilesList")),
                    resp,
                ));
            }
        }

        let bs = resp.into_body();

        let response: ListResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        for file in response.files {
            let path = build_abs_path(&normalize_root(&self.path), &file.name);

            let entry = if file.ty == "dir" {
                let path = format!("{path}/");
                Entry::new(&path, MetadataBuilder::dir().build())
            } else {
                let mut m = MetadataBuilder::file(file.size);
                m.content_type(file.content_type)
                    .last_modified(Timestamp::from_millisecond(file.modified)?);
                Entry::new(&path, m.build())
            };

            ctx.entries.push_back(entry);
        }

        ctx.done = true;

        Ok(())
    }
}
