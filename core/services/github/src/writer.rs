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
use http::StatusCode;

use super::core::parse_error;
use super::core::{ErrorContext, GithubCore};
use opendal_core::raw::*;
use opendal_core::*;

pub type GithubWriters = oio::OneShotWriter<GithubWriter>;

pub struct GithubWriter {
    core: Arc<GithubCore>,
    ctx: OperationContext,
    path: String,
}

impl GithubWriter {
    pub fn new(core: Arc<GithubCore>, ctx: OperationContext, path: String) -> Self {
        GithubWriter { core, ctx, path }
    }

    fn parse_metadata(content: &super::core::Entry) -> Result<Metadata> {
        let mode = if content.type_field == "dir" {
            EntryMode::DIR
        } else {
            EntryMode::FILE
        };

        let mut meta = if mode == EntryMode::FILE {
            MetadataBuilder::file(content.size)
        } else {
            MetadataBuilder::dir()
        };
        if mode == EntryMode::FILE {
            meta.etag(&content.sha);
        }

        Ok(meta.build())
    }
}

impl oio::OneShotWrite for GithubWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let resp = self.core.upload(&self.ctx, &self.path, bs).await?;

        let status = resp.status();

        match status {
            StatusCode::OK | StatusCode::CREATED => {
                let body = resp.into_body();
                let content_resp: super::core::ContentResponse =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;
                let metadata = GithubWriter::parse_metadata(&content_resp.content)?;
                Ok(metadata)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateOrUpdateFileContents")),
                resp,
            )),
        }
    }
}
