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

use super::core::GoosefsCore;
use opendal_core::ErrorKind;
use opendal_core::MetadataBuilder;
use opendal_core::Result;
use opendal_core::raw::oio::Entry;
use opendal_core::raw::*;

pub struct GoosefsLister {
    core: Arc<GoosefsCore>,
    path: String,
}

impl GoosefsLister {
    pub(super) fn new(core: Arc<GoosefsCore>, path: &str) -> Self {
        GoosefsLister {
            core,
            path: path.to_string(),
        }
    }
}

impl oio::PageList for GoosefsLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        // OpenDAL's list contract (see behavior tests `list_empty_dir` /
        // `list_nested_dir`) requires that when a caller lists an
        // **existing** directory, the directory itself is the first
        // entry returned. GooseFS `listStatus(dir)` — like HDFS / Alluxio
        // — only returns the directory's children, not the directory
        // node itself, so we synthesize the self-entry here.
        //
        // Importantly, the self-entry must be omitted when the target
        // directory does NOT exist (see behavior test
        // `list_non_exist_dir` which asserts an empty result). So we
        // first attempt the list; only on success do we prepend the
        // synthetic self-entry.
        let result = self.core.list_status(&self.path).await;

        match result {
            Ok(file_infos) => {
                ctx.done = true;

                // Synthesize the self-entry for non-empty directory
                // paths. Listing the backend root ("") is valid but has
                // no "self" entry in OpenDAL semantics, so we skip the
                // synthesis for it.
                if !self.path.is_empty() && self.path.ends_with('/') {
                    ctx.entries
                        .push_back(Entry::new(&self.path, MetadataBuilder::dir().build()));
                }

                for file_info in file_infos {
                    let (rel_path, metadata) = self.core.file_info_to_entry(&file_info)?;
                    // Guard against double-emitting the self-entry if
                    // GooseFS ever decides to include `self.path` in the
                    // listing response (it currently doesn't, but the
                    // semantics are not documented as stable).
                    if rel_path == self.path {
                        continue;
                    }
                    ctx.entries.push_back(Entry::new(&rel_path, metadata));
                }

                Ok(())
            }
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    // Non-existent directory: return an empty page
                    // without a synthetic self-entry.
                    ctx.done = true;
                    return Ok(());
                }
                Err(e)
            }
        }
    }
}
