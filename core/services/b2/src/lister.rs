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

use super::core::ListFileNamesResponse;
use super::core::parse_error;
use super::core::parse_file_info;
use super::core::{B2Core, ErrorContext};
use opendal_core::raw::*;
use opendal_core::*;

pub struct B2Lister {
    core: Arc<B2Core>,
    ctx: OperationContext,

    path: String,
    delimiter: Option<&'static str>,
    limit: Option<usize>,

    /// B2 starts listing **after** this specified key
    start_after: Option<String>,
}

impl B2Lister {
    pub fn new(
        core: Arc<B2Core>,
        ctx: OperationContext,
        path: &str,
        recursive: bool,
        limit: Option<usize>,
        start_after: Option<&str>,
    ) -> Self {
        let delimiter = if recursive { None } else { Some("/") };
        Self {
            core,
            ctx,

            path: path.to_string(),
            delimiter,
            limit,
            start_after: start_after.map(String::from),
        }
    }
}

impl oio::PageList for B2Lister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self
            .core
            .list_file_names(
                &self.ctx,
                Some(&self.path),
                self.delimiter,
                self.limit,
                if ctx.token.is_empty() {
                    self.start_after
                        .as_ref()
                        .map(|v| build_abs_path(&self.core.root, v))
                } else {
                    Some(ctx.token.clone())
                },
            )
            .await?;

        if resp.status() != http::StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListFileNames")),
                resp,
            ));
        }

        let bs = resp.into_body();

        let output: ListFileNamesResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        if let Some(token) = output.next_file_name {
            ctx.token = token;
        } else {
            ctx.done = true;
        }

        for file in output.files {
            if let Some(start_after) = self.start_after.clone()
                && build_abs_path(&self.core.root, &start_after) == file.file_name
            {
                continue;
            }
            let file_name = file.file_name.clone();
            let metadata = parse_file_info(&file);

            ctx.entries.push_back(oio::Entry::new(
                &build_rel_path(&self.core.root, &file_name),
                metadata,
            ))
        }

        Ok(())
    }
}
