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

pub struct AzdlsDeleter {
    core: Arc<AzdlsCore>,
    ctx: OperationContext,
}

impl AzdlsDeleter {
    pub fn new(core: Arc<AzdlsCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::OneShotDelete for AzdlsDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        let resp = if args.recursive() {
            self.core
                .azdls_recursive_delete(&self.ctx, &path, &args)
                .await?
        } else {
            self.core.azdls_delete(&self.ctx, &path, &args).await?
        };

        let status = resp.status();
        let error_ctx = ErrorContext::new(if args.recursive() {
            ServiceOperation("RecursiveDeletePath")
        } else {
            ServiceOperation("DeletePath")
        })
        .with_delete_match_condition(args.if_match().is_some());

        match status {
            StatusCode::OK => Ok(()),
            StatusCode::NOT_FOUND if args.if_match().is_none() => Ok(()),
            _ => Err(parse_error(error_ctx, resp)),
        }
    }
}
