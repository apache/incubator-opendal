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

/// GcsLister takes over task of listing objects and
/// helps walking directory
pub struct GcsLister {
    core: Arc<GcsCore>,
    ctx: OperationContext,

    path: String,
    delimiter: &'static str,
    limit: Option<usize>,

    /// Filter results to objects whose names are lexicographically
    /// **equal to or after** startOffset
    start_after: Option<String>,
}

impl GcsLister {
    /// Generate a new directory walker
    pub fn new(
        core: Arc<GcsCore>,
        ctx: OperationContext,
        path: &str,
        recursive: bool,
        limit: Option<usize>,
        start_after: Option<&str>,
    ) -> Self {
        let delimiter = if recursive { "" } else { "/" };
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

impl oio::PageList for GcsLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self
            .core
            .gcs_list_objects(
                &self.ctx,
                &self.path,
                &ctx.token,
                self.delimiter,
                self.limit,
                if ctx.token.is_empty() {
                    self.start_after.clone()
                } else {
                    None
                },
            )
            .await?;

        if !resp.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListObjects")),
                resp,
            ));
        }
        let bytes = resp.into_body();

        let output: ListResponse =
            serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;

        if let Some(token) = &output.next_page_token {
            ctx.token.clone_from(token);
        } else {
            ctx.done = true;
        }

        for prefix in output.prefixes {
            let de = oio::Entry::new(
                &build_rel_path(&self.core.root, &prefix),
                MetadataBuilder::dir().build(),
            );

            ctx.entries.push_back(de);
        }

        for object in output.items {
            // exclude the inclusive start_after itself
            let mut path = build_rel_path(&self.core.root, &object.name);
            if path.is_empty() {
                path = "/".to_string();
            }
            if self.start_after.as_ref() == Some(&path) {
                continue;
            }

            let size = object.size.parse().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "parse u64 from list response").set_source(e)
            })?;
            let mut meta = if path.ends_with('/') {
                MetadataBuilder::dir()
            } else {
                MetadataBuilder::file(size)
            };

            // set metadata fields
            meta.content_md5(object.md5_hash.as_str());
            meta.etag(object.etag.as_str());
            if !object.generation.is_empty() {
                meta.version(&object.generation);
            }

            if !object.content_type.is_empty() {
                meta.content_type(&object.content_type);
            }

            meta.last_modified(object.updated.parse::<Timestamp>()?);

            let de = oio::Entry::with(path, meta.build());

            ctx.entries.push_back(de);
        }

        Ok(())
    }
}
