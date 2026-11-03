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

use super::core::GithubCore;
use opendal_core::raw::oio::Entry;
use opendal_core::raw::*;
use opendal_core::*;

pub struct GithubLister {
    core: Arc<GithubCore>,
    ctx: OperationContext,

    path: String,
    recursive: bool,
}

impl GithubLister {
    pub fn new(core: Arc<GithubCore>, ctx: OperationContext, path: &str, recursive: bool) -> Self {
        Self {
            core,
            ctx,

            path: path.to_string(),
            recursive,
        }
    }
}

impl oio::PageList for GithubLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self.core.list(&self.ctx, &self.path).await?;

        // Record whether there is a dir in the list so that we need recursive later.
        let has_dir = resp.entries.iter().any(|e| e.type_field == "dir");

        ctx.done = true;

        if !self.recursive || !has_dir {
            for entry in resp.entries {
                let path = build_rel_path(&self.core.root, &entry.path);
                let entry = if entry.type_field == "dir" {
                    let path = format!("{path}/");
                    Entry::new(&path, MetadataBuilder::dir().build())
                } else {
                    if path.ends_with(".gitkeep") {
                        continue;
                    }
                    let mut m = MetadataBuilder::file(entry.size);
                    m.etag(entry.sha);
                    Entry::new(&path, m.build())
                };
                ctx.entries.push_back(entry);
            }
            if !self.path.ends_with('/') {
                ctx.entries.push_back(Entry::new(
                    &format!("{}/", self.path),
                    MetadataBuilder::dir().build(),
                ));
            }
            return Ok(());
        }

        // if recursive is true and there is a dir in the list, we need to list it recursively.

        let tree = self
            .core
            .list_with_recursive(&self.ctx, &resp.git_url)
            .await?;
        for t in tree {
            let path = if self.path == "/" {
                t.path
            } else {
                format!("{}/{}", self.path.trim_end_matches('/'), t.path)
            };
            let entry = if t.type_field == "tree" {
                let path = format!("{path}/");
                Entry::new(&path, MetadataBuilder::dir().build())
            } else {
                if path.ends_with(".gitkeep") {
                    continue;
                }
                let size = t.size.ok_or_else(|| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "github tree response does not contain blob size",
                    )
                })?;
                let mut m = MetadataBuilder::file(size);
                m.etag(t.sha);
                Entry::new(&path, m.build())
            };
            ctx.entries.push_back(entry);
        }
        if !self.path.ends_with('/') {
            ctx.entries.push_back(Entry::new(
                &format!("{}/", self.path),
                MetadataBuilder::dir().build(),
            ));
        }

        Ok(())
    }
}
