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

use std::collections::HashMap;
use std::sync::Arc;

use http::StatusCode;

use super::backend::gist_file_name;
use super::core::ErrorContext;
use super::core::GistCore;
use super::core::UpdateFileRequest;
use super::core::UpdateFilesRequest;
use super::core::parse_error;
use opendal_core::raw::*;
use opendal_core::*;

/// Deleter returned by this backend.
pub struct GistDeleter {
    core: Arc<GistCore>,
    ctx: OperationContext,
}

impl GistDeleter {
    pub(super) fn new(core: Arc<GistCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::OneShotDelete for GistDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let name = gist_file_name(&path)?;

        // Removing a file is part of an edit (`null` deletes the entry), so the
        // current file list decides whether there is anything to delete and
        // keeps deleting a missing file a no-op.
        if self
            .core
            .get_file(&self.ctx, name, Operation::Delete)
            .await?
            .is_none()
        {
            return Ok(());
        }

        let mut files: HashMap<String, Option<UpdateFileRequest>> = HashMap::new();
        files.insert(name.to_string(), None);

        let resp = self
            .core
            .update_files(&self.ctx, Operation::Delete, UpdateFilesRequest { files })
            .await?;

        match resp.status() {
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UpdateGist")),
                resp,
            )),
        }
    }
}
