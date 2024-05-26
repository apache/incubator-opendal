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

use super::core::SeafileCore;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub struct SeafileReader {
    core: Arc<SeafileCore>,

    path: String,
    _op: OpRead,
}

impl SeafileReader {
    pub fn new(core: Arc<SeafileCore>, path: &str, op: OpRead) -> Self {
        SeafileReader {
            core,
            path: path.to_string(),
            _op: op,
        }
    }
}

impl oio::Read for SeafileReader {
    async fn read_at(&self, offset: u64, size: usize) -> Result<Buffer> {
        let range = BytesRange::new(offset, Some(size as u64));

        let resp = self.core.download_file(&self.path, range).await?;

        let status = resp.status();

        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => Ok(resp.into_body()),
            _ => Err(parse_error(resp).await?),
        }
    }
}
