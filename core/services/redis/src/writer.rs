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

use opendal_core::raw::*;
use opendal_core::*;

use super::core::RedisCore;

pub struct RedisWriter {
    core: Arc<RedisCore>,
    path: String,
    buffer: oio::QueueBuf,
}

impl RedisWriter {
    pub fn new(core: Arc<RedisCore>, path: String) -> Self {
        Self {
            core,
            path,
            buffer: oio::QueueBuf::new(),
        }
    }
}

impl oio::Write for RedisWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.buffer.push(bs);
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let buf = self.buffer.clone().collect();
        let length = buf.len() as u64;
        self.core.set(&self.path, buf).await?;

        let metadata = if self.path.ends_with('/') {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::file(length)
        };
        Ok(metadata.build())
    }

    async fn abort(&mut self) -> Result<()> {
        self.buffer.clear();
        Ok(())
    }
}
