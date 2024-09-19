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

use super::core::*;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub struct DropboxLister {
    core: Arc<DropboxCore>,
    path: String,
    recursive: bool,
    limit: Option<usize>,
}

impl DropboxLister {
    pub fn new(
        core: Arc<DropboxCore>,
        path: String,
        recursive: bool,
        limit: Option<usize>,
    ) -> Self {
        Self {
            core,
            path,
            recursive,
            limit,
        }
    }
}

impl oio::PageList for DropboxLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        // The token is set when obtaining entries and returning `has_more` flag.
        // When the token exists, we should retrieve more entries using the Dropbox continue API.
        // Refer: https://www.dropbox.com/developers/documentation/http/documentation#files-list_folder-continue
        let response = if !ctx.token.is_empty() {
            self.core.dropbox_list_continue(&ctx.token).await?
        } else {
            self.core
                .dropbox_list(&self.path, self.recursive, self.limit)
                .await?
        };

        let status_code = response.status();

        if !status_code.is_success() {
            let error = parse_error(response).await?;

            let result = match error.kind() {
                ErrorKind::NotFound => Ok(()),
                _ => Err(error),
            };

            ctx.done = true;
            return result;
        }

        // List "dir" should only return "dir/".
        if !self.path.ends_with('/') {
            let mut path = self.path.clone();
            path.push('/');
            ctx.entries
                .push_back(oio::Entry::with(path, Metadata::new(EntryMode::DIR)));

            ctx.done = true;
            return Ok(());
        }

        let bytes = response.into_body();
        let decoded_response: DropboxListResponse =
            serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;

        for entry in decoded_response.entries {
            let entry_mode = match entry.tag.as_str() {
                "file" => EntryMode::FILE,
                "folder" => EntryMode::DIR,
                _ => EntryMode::Unknown,
            };

            let mut name = entry.name;
            let mut meta = Metadata::new(entry_mode);

            // Dropbox will return folder names that do not end with '/'.
            if entry_mode == EntryMode::DIR && !name.ends_with('/') {
                name.push('/');
            }

            // The behavior here aligns with Dropbox's stat function.
            if entry_mode == EntryMode::FILE {
                let date_utc_last_modified = parse_datetime_from_rfc3339(&entry.client_modified)?;
                meta.set_last_modified(date_utc_last_modified);

                if let Some(size) = entry.size {
                    meta.set_content_length(size);
                }
            }

            // Dropbox will return name without parent path, we need contact it.
            let path = format!("{}{}", self.path, name);
            ctx.entries.push_back(oio::Entry::with(path, meta));
        }

        if decoded_response.has_more {
            ctx.token = decoded_response.cursor;
            ctx.done = false;
        } else {
            ctx.done = true;
        }
        Ok(())
    }
}
