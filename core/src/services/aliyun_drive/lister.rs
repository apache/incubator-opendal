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
use chrono::Utc;

use crate::raw::*;
use crate::EntryMode;
use crate::Error;
use crate::ErrorKind;
use crate::Metadata;
use crate::Result;

use self::oio::Entry;

use super::core::AliyunDriveCore;
use super::core::AliyunDriveFile;
use super::core::AliyunDriveFileList;

pub struct AliyunDriveLister {
    core: Arc<AliyunDriveCore>,

    parent: Option<AliyunDriveParent>,
    limit: Option<usize>,
}

pub struct AliyunDriveParent {
    pub parent_path: String,
    pub parent_file_id: String,
}

impl AliyunDriveLister {
    pub fn new(
        core: Arc<AliyunDriveCore>,
        parent: Option<AliyunDriveParent>,
        limit: Option<usize>,
    ) -> Self {
        AliyunDriveLister {
            core,
            parent,
            limit,
        }
    }
}

impl oio::PageList for AliyunDriveLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let Some(parent) = &self.parent else {
            ctx.done = true;
            return Ok(());
        };

        let offset = if ctx.token.is_empty() {
            None
        } else {
            Some(ctx.token.clone())
        };

        let res = self
            .core
            .list(&parent.parent_file_id, self.limit, offset)
            .await;
        let res = match res {
            Err(err) if err.kind() == ErrorKind::NotFound => {
                ctx.done = true;
                None
            }
            Err(err) => return Err(err),
            Ok(res) => Some(res),
        };

        let Some(res) = res else {
            return Ok(());
        };

        let result: AliyunDriveFileList =
            serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;

        let n = result.items.len();

        for item in result.items {
            let res = self.core.get(&item.file_id).await?;
            let file: AliyunDriveFile =
                serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;

            let path = if parent.parent_path.starts_with('/') {
                build_abs_path(&parent.parent_path, &file.name)
            } else {
                build_abs_path(&format!("/{}", &parent.parent_path), &file.name)
            };

            let (path, md) = if file.path_type == "folder" {
                let path = format!("{}/", path);
                let meta = Metadata::new(EntryMode::DIR).with_last_modified(
                    file.updated_at
                        .parse::<chrono::DateTime<Utc>>()
                        .map_err(|e| {
                            Error::new(ErrorKind::Unexpected, "parse last modified time")
                                .set_source(e)
                        })?,
                );
                (path, meta)
            } else {
                let mut meta = Metadata::new(EntryMode::FILE).with_last_modified(
                    file.updated_at
                        .parse::<chrono::DateTime<Utc>>()
                        .map_err(|e| {
                            Error::new(ErrorKind::Unexpected, "parse last modified time")
                                .set_source(e)
                        })?,
                );
                if let Some(v) = file.size {
                    meta = meta.with_content_length(v);
                }
                if let Some(v) = file.content_type {
                    meta = meta.with_content_type(v);
                }
                (path, meta)
            };

            ctx.entries.push_back(Entry::new(&path, md));
        }

        if self.limit.is_some_and(|x| x < n) || result.next_marker.is_none() {
            ctx.done = true;
        }

        if let Some(marker) = result.next_marker {
            if marker.is_empty() {
                ctx.done = true;
            }
            ctx.token = marker;
        }
        Ok(())
    }
}
