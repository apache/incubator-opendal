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
use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub struct AzfileDeleter {
    core: Arc<AzfileCore>,
    ctx: OperationContext,
}

impl AzfileDeleter {
    pub fn new(core: Arc<AzfileCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::OneShotDelete for AzfileDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let resp = if path.ends_with('/') {
            self.core.azfile_delete_dir(&self.ctx, &path).await?
        } else {
            self.core.azfile_delete_file(&self.ctx, &path).await?
        };

        let status = resp.status();
        match status {
            StatusCode::ACCEPTED | StatusCode::NOT_FOUND => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(if path.ends_with('/') {
                    ServiceOperation("DeleteDirectory")
                } else {
                    ServiceOperation("DeleteFile")
                }),
                resp,
            )),
        }
    }
}
