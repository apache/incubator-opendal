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

use super::core::DropboxCore;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub struct DropboxWriter {
    core: Arc<DropboxCore>,
    op: OpWrite,
    path: String,
}

impl DropboxWriter {
    pub fn new(core: Arc<DropboxCore>, op: OpWrite, path: String) -> Self {
        DropboxWriter { core, op, path }
    }
}

impl oio::OneShotWrite for DropboxWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let resp = self
            .core
            .dropbox_update(&self.path, Some(bs.len()), &self.op, bs)
            .await?;
        let status = resp.status();
        match status {
            StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }
}
