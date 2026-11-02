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

use super::core::VercelBlobCore;
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::{OpDelete, oio};

pub struct VercelBlobDeleter {
    core: Arc<VercelBlobCore>,
    ctx: OperationContext,
}

impl VercelBlobDeleter {
    pub fn new(core: Arc<VercelBlobCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::OneShotDelete for VercelBlobDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        self.core.vercel_delete_blob(&self.ctx, &path).await
    }
}
