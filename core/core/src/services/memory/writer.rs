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
use crate::raw::oio;
use crate::raw::*;
use crate::*;

pub struct MemoryWriter {
    core: Arc<MemoryCore>,
    path: String,
    op: OpWrite,
    buf: Option<oio::QueueBuf>,
}

impl MemoryWriter {
    pub fn new(core: Arc<MemoryCore>, path: String, op: OpWrite) -> Self {
        Self {
            core,
            path,
            op,
            buf: None,
        }
    }
}

impl oio::Write for MemoryWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let mut buf = self.buf.take().unwrap_or_default();
        buf.push(bs);
        self.buf = Some(buf);
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let buf = self.buf.take().unwrap_or_default();
        let content = buf.collect();

        let mut metadata = MetadataBuilder::file(content.len() as u64);

        if let Some(v) = self.op.cache_control() {
            metadata.cache_control(v);
        }
        if let Some(v) = self.op.content_disposition() {
            metadata.content_disposition(v);
        }
        if let Some(v) = self.op.content_type() {
            metadata.content_type(v);
        }
        if let Some(v) = self.op.content_encoding() {
            metadata.content_encoding(v);
        }

        let metadata = metadata.build();

        let value = MemoryValue {
            metadata: metadata.clone(),
            content,
        };

        if self.op.if_not_exists() {
            self.core.set_if_not_exists(&self.path, value)?;
        } else {
            self.core.set(&self.path, value)?;
        }

        Ok(metadata)
    }

    async fn abort(&mut self) -> Result<()> {
        self.buf = None;
        Ok(())
    }
}
