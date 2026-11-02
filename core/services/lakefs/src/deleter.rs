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
use opendal_core::raw::*;
use opendal_core::*;

use super::core::parse_error;
use super::core::*;

pub struct LakefsDeleter {
    core: Arc<LakefsCore>,
    ctx: OperationContext,
}

impl LakefsDeleter {
    pub fn new(core: Arc<LakefsCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::OneShotDelete for LakefsDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        // This would delete the bucket, do not perform
        if self.core.root == "/" && path == "/" {
            return Ok(());
        }

        let resp = self.core.delete_object(&self.ctx, &path, &args).await?;

        let status = resp.status();

        match status {
            StatusCode::NO_CONTENT => Ok(()),
            StatusCode::NOT_FOUND => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteObject")),
                resp,
            )),
        }
    }
}
