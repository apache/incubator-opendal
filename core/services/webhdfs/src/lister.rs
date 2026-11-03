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

use super::core::parse_error;
use super::core::{ErrorContext, WebhdfsCore};
use super::message::*;
use opendal_core::raw::oio;
use opendal_core::raw::*;
use opendal_core::*;

pub struct WebhdfsLister {
    core: Arc<WebhdfsCore>,
    ctx: OperationContext,
    path: String,
}

impl WebhdfsLister {
    pub fn new(core: Arc<WebhdfsCore>, ctx: OperationContext, path: &str) -> Self {
        Self {
            core,
            ctx,
            path: path.to_string(),
        }
    }
}

impl oio::PageList for WebhdfsLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let file_status = if self.core.disable_list_batch {
            let resp = self.core.webhdfs_list_status(&self.ctx, &self.path).await?;
            match resp.status() {
                StatusCode::OK => {
                    ctx.done = true;
                    ctx.entries.push_back(oio::Entry::new(
                        format!("{}/", self.path).as_str(),
                        MetadataBuilder::dir().build(),
                    ));

                    let bs = resp.into_body();
                    serde_json::from_reader::<_, FileStatusesWrapper>(bs.reader())
                        .map_err(new_json_deserialize_error)?
                        .file_statuses
                        .file_status
                }
                StatusCode::NOT_FOUND => {
                    ctx.done = true;
                    return Ok(());
                }
                _ => {
                    return Err(parse_error(
                        ErrorContext::new(ServiceOperation("ListStatus")),
                        resp,
                    ));
                }
            }
        } else {
            let resp = self
                .core
                .webhdfs_list_status_batch(&self.ctx, &self.path, &ctx.token)
                .await?;
            match resp.status() {
                StatusCode::OK => {
                    let bs = resp.into_body();
                    let res: DirectoryListingWrapper =
                        serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                    let directory_listing = res.directory_listing;
                    let file_statuses = directory_listing.partial_listing.file_statuses.file_status;

                    if directory_listing.remaining_entries == 0 {
                        ctx.entries.push_back(oio::Entry::new(
                            format!("{}/", self.path).as_str(),
                            MetadataBuilder::dir().build(),
                        ));

                        ctx.done = true;
                    } else if !file_statuses.is_empty() {
                        ctx.token
                            .clone_from(&file_statuses.last().unwrap().path_suffix);
                    }

                    file_statuses
                }
                StatusCode::NOT_FOUND => {
                    ctx.done = true;
                    return Ok(());
                }
                _ => {
                    return Err(parse_error(
                        ErrorContext::new(ServiceOperation("ListStatusBatch")),
                        resp,
                    ));
                }
            }
        };

        for status in file_status {
            let mut path = if self.path.is_empty() {
                status.path_suffix.to_string()
            } else {
                format!("{}/{}", self.path, status.path_suffix)
            };

            let meta = match status.ty {
                FileStatusType::Directory => MetadataBuilder::dir().build(),
                FileStatusType::File => {
                    let mut metadata = MetadataBuilder::file(status.length);
                    metadata.last_modified(Timestamp::from_millisecond(status.modification_time)?);
                    metadata.build()
                }
            };

            if meta.mode().is_file() {
                path = path.trim_end_matches('/').to_string();
            }
            if meta.mode().is_dir() {
                path += "/"
            }
            let entry = oio::Entry::new(&path, meta);
            ctx.entries.push_back(entry);
        }

        Ok(())
    }
}
