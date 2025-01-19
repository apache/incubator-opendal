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

use super::core::AzfileCore;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub type AzfileWriters = TwoWays<oio::OneShotWriter<AzfileWriter>, oio::AppendWriter<AzfileWriter>>;

pub struct AzfileWriter {
    core: Arc<AzfileCore>,
    op: OpWrite,
    path: String,
}

impl AzfileWriter {
    pub fn new(core: Arc<AzfileCore>, op: OpWrite, path: String) -> Self {
        AzfileWriter { core, op, path }
    }
}

impl oio::OneShotWrite for AzfileWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let resp = self
            .core
            .azfile_create_file(&self.path, bs.len(), &self.op)
            .await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::CREATED => {}
            _ => {
                return Err(parse_error(resp).with_operation("Backend::azfile_create_file"));
            }
        }

        let resp = self
            .core
            .azfile_update(&self.path, bs.len() as u64, 0, bs)
            .await?;
        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::CREATED => Ok(Metadata::default()),
            _ => Err(parse_error(resp).with_operation("Backend::azfile_update")),
        }
    }
}

impl oio::AppendWrite for AzfileWriter {
    async fn offset(&self) -> Result<u64> {
        let resp = self.core.azfile_get_file_properties(&self.path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(parse_content_length(resp.headers())?.unwrap_or_default()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn append(&self, offset: u64, size: u64, body: Buffer) -> Result<()> {
        let resp = self
            .core
            .azfile_update(&self.path, size, offset, body)
            .await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::CREATED => Ok(()),
            _ => Err(parse_error(resp).with_operation("Backend::azfile_update")),
        }
    }
}
