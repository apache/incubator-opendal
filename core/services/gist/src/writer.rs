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
use super::core::Gist;
use super::core::GistCore;
use super::core::UpdateFileRequest;
use super::core::UpdateFilesRequest;
use super::core::parse_error;
use super::core::parse_metadata;
use opendal_core::raw::*;
use opendal_core::*;

/// Writer returned by this backend.
pub struct GistWriter {
    core: Arc<GistCore>,
    ctx: OperationContext,
    path: String,
}

impl GistWriter {
    pub(super) fn new(core: Arc<GistCore>, ctx: OperationContext, path: String) -> Self {
        Self { core, ctx, path }
    }
}

impl oio::OneShotWrite for GistWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let name = gist_file_name(&self.path)?;

        // The Gist API carries file content as a JSON string, so non UTF-8
        // content cannot be represented at all.
        let content = String::from_utf8(bs.to_bytes().to_vec()).map_err(|_| {
            Error::new(ErrorKind::Unsupported, "gist files must be valid UTF-8")
                .with_context("path", name)
        })?;

        let mut files = HashMap::new();
        files.insert(name.to_string(), Some(UpdateFileRequest { content }));

        let resp = self
            .core
            .update_files(&self.ctx, Operation::Write, UpdateFilesRequest { files })
            .await?;

        match resp.status() {
            StatusCode::OK => {
                let body = resp.into_body();
                let gist: Gist =
                    serde_json::from_slice(&body.to_bytes()).map_err(new_json_deserialize_error)?;

                let file = gist.files.get(name).ok_or_else(|| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "gist update response does not contain the written file",
                    )
                    .with_context("path", name)
                })?;

                Ok(parse_metadata(file))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UpdateGist")),
                resp,
            )),
        }
    }
}
