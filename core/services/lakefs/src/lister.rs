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

use super::core::LakefsListResponse;
use super::core::parse_error;
use super::core::{ErrorContext, LakefsCore};

pub struct LakefsLister {
    core: Arc<LakefsCore>,
    ctx: OperationContext,
    path: String,
    delimiter: &'static str,
    amount: Option<usize>,
    after: Option<String>,
}

impl LakefsLister {
    pub fn new(
        core: Arc<LakefsCore>,
        ctx: OperationContext,
        path: String,
        amount: Option<usize>,
        after: Option<&str>,
        recursive: bool,
    ) -> Self {
        let delimiter = if recursive { "" } else { "/" };
        Self {
            core,
            ctx,
            path,
            delimiter,
            amount,
            after: after.map(String::from),
        }
    }
}

impl oio::PageList for LakefsLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let response = self
            .core
            .list_objects(
                &self.ctx,
                &self.path,
                self.delimiter,
                &self.amount,
                // start_after applies to the first page; later pages resume from the
                // cursor the previous response returned.
                if ctx.token.is_empty() {
                    self.after.clone()
                } else {
                    Some(ctx.token.clone())
                },
            )
            .await?;

        let status_code = response.status();
        if !status_code.is_success() {
            let error = parse_error(ErrorContext::new(ServiceOperation("ListObjects")), response);
            return Err(error);
        }

        let bytes = response.into_body();

        let decoded_response: LakefsListResponse =
            serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;

        let pagination = decoded_response.pagination;
        ctx.done = !pagination.has_more;
        ctx.token = pagination.next_offset;

        for status in decoded_response.results {
            let entry_type = match status.path_type.as_str() {
                "common_prefix" => EntryMode::DIR,
                "object" => EntryMode::FILE,
                _ => EntryMode::Unknown,
            };

            let mut meta = match entry_type {
                EntryMode::FILE => status
                    .size_bytes
                    .map_or_else(MetadataBuilder::unknown, MetadataBuilder::file),
                EntryMode::DIR => MetadataBuilder::dir(),
                EntryMode::Unknown => MetadataBuilder::unknown(),
            };

            if status.mtime != 0 {
                meta.last_modified(Timestamp::from_second(status.mtime).unwrap());
            }

            let path = if entry_type == EntryMode::DIR {
                format!("{}/", status.path)
            } else {
                status.path.clone()
            };

            ctx.entries.push_back(oio::Entry::new(
                &build_rel_path(&self.core.root, &path),
                meta.build(),
            ));
        }

        Ok(())
    }
}
