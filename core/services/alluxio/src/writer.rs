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

use super::core::AlluxioCore;
use opendal_core::raw::*;
use opendal_core::*;

pub type AlluxioWriters = AlluxioWriter;

pub struct AlluxioWriter {
    core: Arc<AlluxioCore>,
    ctx: OperationContext,

    _op: OpWrite,
    path: String,
    stream_id: Option<u64>,
}

impl AlluxioWriter {
    pub fn new(core: Arc<AlluxioCore>, ctx: OperationContext, _op: OpWrite, path: String) -> Self {
        AlluxioWriter {
            core,
            ctx,
            _op,
            path,
            stream_id: None,
        }
    }
}

impl oio::Write for AlluxioWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let stream_id = match self.stream_id {
            Some(stream_id) => stream_id,
            None => {
                let stream_id = self.core.create_file(&self.ctx, &self.path).await?;
                self.stream_id = Some(stream_id);
                stream_id
            }
        };
        self.core.write(&self.ctx, stream_id, bs).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let Some(stream_id) = self.stream_id else {
            return Ok(MetadataBuilder::unknown().build());
        };
        self.core.close(&self.ctx, stream_id).await?;

        Ok(MetadataBuilder::unknown().build())
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "AlluxioWriter doesn't support abort",
        ))
    }
}
