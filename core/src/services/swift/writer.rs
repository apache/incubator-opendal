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

use super::core::SwiftCore;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub struct SwiftWriter {
    core: Arc<SwiftCore>,
    path: String,
}

impl SwiftWriter {
    pub fn new(core: Arc<SwiftCore>, _op: OpWrite, path: String) -> Self {
        SwiftWriter { core, path }
    }
}

impl oio::OneShotWrite for SwiftWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let resp = self
            .core
            .swift_create_object(&self.path, bs.len() as u64, bs)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }
}
