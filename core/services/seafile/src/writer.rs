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

use super::core::parse_error;
use super::core::{ErrorContext, SeafileCore};
use opendal_core::raw::*;
use opendal_core::*;

pub type SeafileWriters = oio::OneShotWriter<SeafileWriter>;

pub struct SeafileWriter {
    core: Arc<SeafileCore>,
    ctx: OperationContext,
    _op: OpWrite,
    path: String,
}

impl SeafileWriter {
    pub fn new(core: Arc<SeafileCore>, ctx: OperationContext, op: OpWrite, path: String) -> Self {
        SeafileWriter {
            core,
            ctx,
            _op: op,
            path,
        }
    }
}

impl oio::OneShotWrite for SeafileWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let resp = self.core.upload_file(&self.ctx, &self.path, bs).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => Ok(MetadataBuilder::unknown().build()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadFile")),
                resp,
            )),
        }
    }
}
