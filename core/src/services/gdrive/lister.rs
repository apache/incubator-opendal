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

use http::StatusCode;

use super::core::GdriveCore;
use super::core::GdriveFileList;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub struct GdriveLister {
    path: String,
    core: Arc<GdriveCore>,
}

impl GdriveLister {
    pub fn new(path: String, core: Arc<GdriveCore>) -> Self {
        Self {
            path: super::core::gdrive_normalize_dir_path(path),
            core,
        }
    }
}

impl oio::PageList for GdriveLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let file_id = self.core.path_cache.get(&self.path).await?;

        let file_id = match file_id {
            Some(file_id) => file_id,
            None => {
                ctx.done = true;
                return Ok(());
            }
        };

        let resp = self
            .core
            .gdrive_list(file_id.as_str(), 100, &ctx.token)
            .await?;

        let bytes = match resp.status() {
            StatusCode::OK => resp.into_body().to_bytes(),
            _ => return Err(parse_error(resp)),
        };

        // Google Drive returns an empty response when attempting to list a non-existent directory.
        if bytes.is_empty() {
            ctx.done = true;
            return Ok(());
        }

        // Include the current directory itself when handling the first page of the listing.
        if ctx.token.is_empty() && !ctx.done {
            let path = build_rel_path(&self.core.root, &self.path);
            let e = oio::Entry::new(&path, Metadata::new(EntryMode::DIR));
            ctx.entries.push_back(e);
        }

        let decoded_response =
            serde_json::from_slice::<GdriveFileList>(&bytes).map_err(new_json_deserialize_error)?;

        if let Some(next_page_token) = decoded_response.next_page_token {
            ctx.token = next_page_token;
        } else {
            ctx.done = true;
        }

        for mut file in decoded_response.files {
            let file_type = if file.mime_type.as_str() == "application/vnd.google-apps.folder" {
                if !file.name.ends_with('/') {
                    file.name += "/";
                }
                EntryMode::DIR
            } else {
                EntryMode::FILE
            };

            let root = &self.core.root;
            let path = format!("{}{}", &self.path, file.name);
            let normalized_path = build_rel_path(root, &path);

            // Update path cache when path doesn't exist.
            // When Google Drive converts a format, for example, Microsoft PowerPoint,
            // Google Drive keeps two entries with the same ID.
            if let Ok(None) = self.core.path_cache.get(&path).await {
                self.core.path_cache.insert(&path, &file.id).await;
            }

            let entry = oio::Entry::new(&normalized_path, Metadata::new(file_type));
            ctx.entries.push_back(entry);
        }

        Ok(())
    }
}
