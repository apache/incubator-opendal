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

use super::core::parse_error;
use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub struct GdriveDeleter {
    core: Arc<GdriveCore>,
    ctx: OperationContext,
}

impl GdriveDeleter {
    pub fn new(core: Arc<GdriveCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::OneShotDelete for GdriveDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let path = build_abs_path(&self.core.root, &path);
        let mut file_id = match self.core.resolve_path(&self.ctx, &path).await? {
            Some(id) => id,
            None => match self
                .core
                .resolve_path_after_refresh(&self.ctx, &path)
                .await?
            {
                Some(id) => id,
                None => return Ok(()),
            },
        };

        let is_dir = if path.ends_with('/') {
            true
        } else {
            let mut resp = self.core.gdrive_stat_by_id(&self.ctx, &file_id).await?;
            if resp.status() == StatusCode::NOT_FOUND {
                file_id = match self
                    .core
                    .resolve_path_after_refresh(&self.ctx, &path)
                    .await?
                {
                    Some(id) => id,
                    None => return Ok(()),
                };
                resp = self.core.gdrive_stat_by_id(&self.ctx, &file_id).await?;
            }

            if resp.status() == StatusCode::NOT_FOUND {
                return Ok(());
            }
            if resp.status() != StatusCode::OK {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("GetFile")),
                    resp,
                ));
            }

            let bs = resp.into_body().to_bytes();
            let file: GdriveFile =
                serde_json::from_slice(&bs).map_err(new_json_deserialize_error)?;
            file.mime_type == "application/vnd.google-apps.folder"
        };

        let mut resp = self.core.gdrive_trash(&self.ctx, &file_id).await?;
        if resp.status() == StatusCode::NOT_FOUND {
            if is_dir {
                self.core.refresh_dir_path(&path).await;
            } else {
                self.core.refresh_path(&path).await;
            }

            file_id = match self.core.resolve_path(&self.ctx, &path).await? {
                Some(id) => id,
                None => return Ok(()),
            };

            resp = self.core.gdrive_trash(&self.ctx, &file_id).await?;
        }

        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }
        if resp.status() != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("TrashFile")),
                resp,
            ));
        }

        if is_dir {
            self.core.invalidate_dir_id(&path).await;
            self.core.record_recent_delete(&path, EntryMode::DIR).await;
        } else {
            self.core.invalidate_file_id(&path).await;
            self.core.record_recent_delete(&path, EntryMode::FILE).await;
        }

        Ok(())
    }
}
