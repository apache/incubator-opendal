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

use opendal_core::raw::oio;
use opendal_core::raw::*;
use opendal_core::*;

use super::core::parse_error;
use super::core::{ErrorContext, OneDriveCore};
use super::graph_model::GraphApiOneDriveListResponse;
use super::graph_model::ItemType;

pub struct OneDriveLister {
    core: Arc<OneDriveCore>,
    ctx: OperationContext,
    path: String,
    op: OpList,
}

impl OneDriveLister {
    const DRIVE_ROOT_PREFIX: &'static str = "/drive/root:";

    pub(crate) fn new(
        path: String,
        core: Arc<OneDriveCore>,
        ctx: OperationContext,
        args: &OpList,
    ) -> Self {
        Self {
            core,
            ctx,
            path,
            op: args.clone(),
        }
    }

    fn is_after_start(&self, path: &str) -> bool {
        self.op
            .start_after()
            .is_none_or(|start_after| path > start_after)
    }
}

impl oio::PageList for OneDriveLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let response = if ctx.token.is_empty() {
            self.core
                .onedrive_list(&self.ctx, &self.path, self.op.limit())
                .await?
        } else {
            self.core
                .onedrive_get_next_list_page(&self.ctx, &ctx.token)
                .await?
        };

        let status_code = response.status();
        if !status_code.is_success() {
            if status_code == http::StatusCode::NOT_FOUND {
                ctx.done = true;
                return Ok(());
            }
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListChildren")),
                response,
            ));
        }

        let bytes = response.into_body();
        let decoded_response: GraphApiOneDriveListResponse =
            serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;

        let list_with_versions = self.op.versions();

        // Include the current directory itself when handling the first page of the listing.
        if ctx.token.is_empty() && !ctx.done {
            // TODO: when listing a directory directly, we could reuse the stat result,
            // cache the result when listing nested directory
            let path = if self.path == "/" {
                "".to_string()
            } else {
                self.path.clone()
            };

            let meta = self
                .core
                .onedrive_stat(&self.ctx, &path, OpStat::default())
                .await?;

            // skip `list_with_versions` intentionally because a folder doesn't have versions

            if self.is_after_start(&path) {
                let entry = oio::Entry::new(&path, meta);
                ctx.entries.push_back(entry);
            }
        }

        if let Some(next_link) = decoded_response.next_link {
            ctx.token = next_link;
        } else {
            ctx.done = true;
        }

        for drive_item in decoded_response.value {
            let name = drive_item.name;
            let parent_path = drive_item.parent_reference.path;
            let parent_path = parent_path
                .strip_prefix(Self::DRIVE_ROOT_PREFIX)
                .unwrap_or("");

            let path = format!("{parent_path}/{name}");
            let mut normalized_path = build_rel_path(self.core.root.as_str(), path.as_str());
            let entry_mode = match drive_item.item_type {
                ItemType::Folder { .. } => EntryMode::DIR,
                ItemType::File { .. } => EntryMode::FILE,
            };

            // Add the trailing `/` because OneDrive returns a directory with the name
            if entry_mode == EntryMode::DIR {
                normalized_path.push('/');
            }

            if !self.is_after_start(&normalized_path) {
                continue;
            }

            let mut meta = if entry_mode == EntryMode::FILE {
                MetadataBuilder::file(drive_item.size.max(0) as u64)
            } else {
                MetadataBuilder::dir()
            };
            meta.etag(drive_item.e_tag);
            let last_modified = drive_item.last_modified_date_time.parse::<Timestamp>()?;
            meta.last_modified(last_modified);
            let meta = meta.build();

            // OneDrive doesn't support expanding versions while listing a directory.
            // Query each file separately when callers request versions.
            if list_with_versions && entry_mode == EntryMode::FILE {
                let versions = self
                    .core
                    .onedrive_list_versions(&self.ctx, &normalized_path)
                    .await?;

                if versions.is_empty() {
                    ctx.entries
                        .push_back(oio::Entry::new(&normalized_path, meta));
                    continue;
                }

                for (index, version) in versions.into_iter().enumerate() {
                    let mut version_meta = MetadataBuilder::file(version.size.max(0) as u64);
                    version_meta
                        .last_modified(version.last_modified_date_time.parse::<Timestamp>()?);
                    version_meta.version(&version.id);
                    version_meta.is_current(Some(index == 0));

                    // OneDrive exposes the ETag only for the current item.
                    if index == 0 {
                        version_meta.etag(meta.etag().unwrap_or_default());
                    }

                    ctx.entries
                        .push_back(oio::Entry::new(&normalized_path, version_meta.build()));
                }
            } else {
                ctx.entries
                    .push_back(oio::Entry::new(&normalized_path, meta));
            }
        }

        Ok(())
    }
}
