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

use super::core::*;
use crate::raw::*;
use crate::services::alluxio::error::parse_error;
use crate::*;

pub struct AlluxioReader {
    core: Arc<AlluxioCore>,

    stream_id: u64,
    _op: OpRead,
}

impl AlluxioReader {
    pub fn new(core: Arc<AlluxioCore>, stream_id: u64, op: OpRead) -> Self {
        AlluxioReader {
            core,
            stream_id,
            _op: op,
        }
    }
}

impl oio::Read for AlluxioReader {
    async fn read_at(&self, offset: u64, limit: usize) -> Result<oio::Buffer> {
        let range = BytesRange::new(offset, Some(limit as u64));

        let resp = self.core.read(self.stream_id, range).await?;

        if !resp.status().is_success() {
            return Err(parse_error(resp).await?);
        }
        Ok(resp.into_body())
    }
}
