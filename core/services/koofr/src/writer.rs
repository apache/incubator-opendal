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

use super::core::File;
use super::core::parse_error;
use super::core::{ErrorContext, KoofrCore};
use opendal_core::raw::*;
use opendal_core::*;

pub type KoofrWriters = oio::OneShotWriter<KoofrWriter>;

pub struct KoofrWriter {
    core: Arc<KoofrCore>,
    ctx: OperationContext,
    path: String,
}

impl KoofrWriter {
    pub fn new(core: Arc<KoofrCore>, ctx: OperationContext, path: String) -> Self {
        KoofrWriter { core, ctx, path }
    }

    fn parse_metadata(file: &File) -> Result<Metadata> {
        let mode = if file.ty == "dir" {
            EntryMode::DIR
        } else {
            EntryMode::FILE
        };

        let mut meta = if mode == EntryMode::FILE {
            MetadataBuilder::file(file.size)
        } else {
            MetadataBuilder::dir()
        };
        meta.content_type(&file.content_type);
        meta.last_modified(Timestamp::from_millisecond(file.modified)?);

        Ok(meta.build())
    }
}

impl oio::OneShotWrite for KoofrWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        self.core.ensure_dir_exists(&self.ctx, &self.path).await?;

        let resp = self.core.put(&self.ctx, &self.path, bs).await?;

        let status = resp.status();

        match status {
            StatusCode::OK | StatusCode::CREATED => {
                let body = resp.into_body();
                let file: File =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;
                let metadata = Self::parse_metadata(&file)?;
                Ok(metadata)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("FilesPut")),
                resp,
            )),
        }
    }
}
