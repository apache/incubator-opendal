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

use super::core::MetainformationResponse;
use super::core::parse_error;
use super::core::parse_info;
use super::core::{ErrorContext, YandexDiskCore};
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::oio::Entry;
use opendal_core::raw::*;

pub struct YandexDiskLister {
    core: Arc<YandexDiskCore>,
    ctx: OperationContext,

    path: String,
    limit: Option<usize>,
}

impl YandexDiskLister {
    pub(super) fn new(
        core: Arc<YandexDiskCore>,
        ctx: OperationContext,
        path: &str,
        limit: Option<usize>,
    ) -> Self {
        YandexDiskLister {
            core,
            ctx,
            path: path.to_string(),
            limit,
        }
    }
}

impl oio::PageList for YandexDiskLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let offset = if ctx.token.is_empty() {
            None
        } else {
            Some(ctx.token.clone())
        };

        let resp = self
            .core
            .metainformation(&self.ctx, &self.path, self.limit, offset)
            .await?;

        if resp.status() == http::StatusCode::NOT_FOUND {
            ctx.done = true;
            return Ok(());
        }

        match resp.status() {
            http::StatusCode::OK => {
                let body = resp.into_body();

                let mut resp: MetainformationResponse =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

                if let Some(embedded) = resp.embedded.take() {
                    let n = embedded.items.len();

                    for mf in embedded.items {
                        let path = mf.path.strip_prefix("disk:");

                        if let Some(path) = path {
                            let mut path = build_rel_path(&self.core.root, path);

                            let md = parse_info(mf)?;

                            if md.mode().is_dir() {
                                path = format!("{path}/");
                            }

                            ctx.entries.push_back(Entry::new(&path, md));
                        };
                    }

                    let current_len = ctx.token.parse::<usize>().unwrap_or(0) + n;

                    if current_len >= embedded.total {
                        ctx.done = true;
                    }

                    ctx.token = current_len.to_string();

                    return Ok(());
                }

                let rel = resp
                    .path
                    .strip_prefix("disk:")
                    .map(|p| build_rel_path(&self.core.root, p));
                if let Some(rel) = rel {
                    let md = parse_info(resp)?;
                    let path = if md.mode().is_dir() {
                        format!("{rel}/")
                    } else {
                        rel
                    };
                    ctx.entries.push_back(Entry::new(&path, md));
                }
                ctx.done = true;

                Ok(())
            }
            http::StatusCode::NOT_FOUND => {
                ctx.done = true;
                Ok(())
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetMetainformation")),
                resp,
            )),
        }
    }
}
