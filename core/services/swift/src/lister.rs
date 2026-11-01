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

use super::core::parse_error;
use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub struct SwiftLister {
    core: Arc<SwiftCore>,
    ctx: OperationContext,
    path: String,
    delimiter: &'static str,
    limit: Option<usize>,
    abs_start_after: Option<String>,
}

impl SwiftLister {
    pub fn new(
        core: Arc<SwiftCore>,
        ctx: OperationContext,
        path: String,
        recursive: bool,
        limit: Option<usize>,
        start_after: Option<String>,
    ) -> Self {
        let delimiter = if recursive { "" } else { "/" };
        // Swift listing names are absolute (root-prefixed) and the lister pages
        // by `marker`, so the start-after must be made absolute as well.
        let abs_start_after =
            start_after.map(|start_after| build_abs_path(&core.root, &start_after));
        Self {
            core,
            ctx,
            path,
            delimiter,
            limit,
            abs_start_after,
        }
    }
}

impl oio::PageList for SwiftLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        // `start_after` applies to the first page only; subsequent pages
        // continue from the previous page's last entry carried in `ctx.token`.
        let marker = if ctx.token.is_empty() {
            self.abs_start_after.as_deref().unwrap_or("")
        } else {
            ctx.token.as_str()
        };
        let response = self
            .core
            .swift_list(&self.ctx, &self.path, self.delimiter, self.limit, marker)
            .await?;

        let status_code = response.status();

        if !status_code.is_success() {
            let error = parse_error(ErrorContext::new(ServiceOperation("ListObjects")), response);
            return Err(error);
        }

        let bytes = response.into_body();
        let decoded_response: Vec<ListOpResponse> =
            serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;

        // Update token and done based on resp.
        if let Some(entry) = decoded_response.last() {
            let path = match entry {
                ListOpResponse::Subdir { subdir } => subdir,
                ListOpResponse::FileInfo { name, .. } => name,
            };
            ctx.token.clone_from(path);
        } else {
            ctx.done = true;
        }

        for status in decoded_response {
            let entry: oio::Entry = match status {
                ListOpResponse::Subdir { subdir } => {
                    let mut path = build_rel_path(self.core.root.as_str(), subdir.as_str());
                    if path.is_empty() {
                        path = "/".to_string();
                    }
                    let meta = MetadataBuilder::dir().build();
                    oio::Entry::with(path, meta)
                }
                ListOpResponse::FileInfo {
                    bytes,
                    hash,
                    name,
                    content_type,
                    mut last_modified,
                } => {
                    let mut path = build_rel_path(self.core.root.as_str(), name.as_str());
                    if path.is_empty() {
                        path = "/".to_string();
                    }
                    let mut meta = if path.ends_with('/') {
                        MetadataBuilder::dir()
                    } else {
                        MetadataBuilder::file(bytes)
                    };
                    meta.content_md5(hash.as_str());

                    // OpenStack Swift returns time without 'Z' at the end,
                    // which causes an error in parse_datetime_from_rfc3339.
                    // we'll change "2023-10-28T19:18:11.682610" to "2023-10-28T19:18:11.682610Z".
                    if !last_modified.ends_with('Z') {
                        last_modified.push('Z');
                    }
                    meta.last_modified(last_modified.parse::<Timestamp>()?);

                    if let Some(content_type) = content_type {
                        meta.content_type(content_type.as_str());
                    }

                    oio::Entry::with(path, meta.build())
                }
            };
            ctx.entries.push_back(entry);
        }
        Ok(())
    }
}
