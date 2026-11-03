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

use self::oio::Entry;
use super::core::AliyunDriveCore;
use super::core::AliyunDriveFile;
use super::core::AliyunDriveFileList;
use bytes::Buf;
use opendal_core::Error;
use opendal_core::ErrorKind;
use opendal_core::MetadataBuilder;
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::*;

pub struct AliyunDriveLister {
    core: Arc<AliyunDriveCore>,
    ctx: OperationContext,

    parent: Option<AliyunDriveParent>,
    path: Option<String>,
    limit: Option<usize>,
}

#[derive(Clone)]
pub struct AliyunDriveParent {
    pub file_id: String,
    pub path: String,
    pub updated_at: String,
}

impl AliyunDriveLister {
    pub fn new_with_path(
        core: Arc<AliyunDriveCore>,
        ctx: OperationContext,
        path: String,
        limit: Option<usize>,
    ) -> Self {
        AliyunDriveLister {
            core,
            ctx,
            parent: None,
            path: Some(path),
            limit,
        }
    }

    async fn parent(&self) -> Result<Option<AliyunDriveParent>> {
        if let Some(parent) = &self.parent {
            return Ok(Some(parent.clone()));
        }

        let Some(path) = &self.path else {
            return Ok(None);
        };

        match self.core.get_by_path(&self.ctx, path).await {
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
            Ok(res) => {
                let file: AliyunDriveFile =
                    serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;
                Ok(Some(AliyunDriveParent {
                    file_id: file.file_id,
                    path: path.to_string(),
                    updated_at: file.updated_at,
                }))
            }
        }
    }
}

impl oio::PageList for AliyunDriveLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let Some(parent) = self.parent().await? else {
            ctx.done = true;
            return Ok(());
        };

        let offset = if ctx.token.is_empty() {
            // Push self into the list result.
            ctx.entries.push_back(Entry::new(&parent.path, {
                let mut metadata = MetadataBuilder::dir();
                metadata.last_modified(parent.updated_at.parse::<Timestamp>().map_err(|e| {
                    Error::new(ErrorKind::Unexpected, "parse last modified time").set_source(e)
                })?);
                metadata.build()
            }));
            None
        } else {
            Some(ctx.token.clone())
        };

        let res = self
            .core
            .list(&self.ctx, &parent.file_id, self.limit, offset)
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

        for item in result.items {
            let (path, mut md) = if item.path_type == "folder" {
                let path = format!("{}{}/", parent.path.trim_start_matches('/'), item.name);
                (path, MetadataBuilder::dir())
            } else {
                let path = format!("{}{}", parent.path.trim_start_matches('/'), item.name);
                let size = item.size.ok_or_else(|| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "aliyun drive list response does not contain file size",
                    )
                })?;
                (path, MetadataBuilder::file(size))
            };

            md.last_modified(item.updated_at.parse::<Timestamp>().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "parse last modified time").set_source(e)
            })?);
            if let Some(v) = item.content_type {
                md.content_type(v);
            }

            ctx.entries.push_back(Entry::new(&path, md.build()));
        }

        let next_marker = result.next_marker.unwrap_or_default();
        if next_marker.is_empty() {
            ctx.done = true;
        } else {
            ctx.token = next_marker;
        }

        Ok(())
    }
}
