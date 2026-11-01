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

use crate::core::{ErrorContext, GcsGrpcCore, parse_generation, parse_status};
use crate::generated::google::storage::v2::DeleteObjectRequest;

pub(super) struct GcsGrpcDeleter {
    core: Arc<GcsGrpcCore>,
    ctx: OperationContext,
}

impl GcsGrpcDeleter {
    pub(super) fn new(core: Arc<GcsGrpcCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::Delete for GcsGrpcDeleter {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        let request = DeleteObjectRequest {
            bucket: self.core.bucket_resource(),
            object: self.core.object_name(path),
            generation: parse_generation(args.version())?,
        };
        let request = self
            .core
            .request(
                &self.ctx,
                request,
                &[("bucket", &self.core.bucket_resource())],
            )
            .await?;
        match self.core.client().delete_object(request).await {
            Ok(_) => Ok(()),
            Err(status) if status.code() == tonic::Code::NotFound => Ok(()),
            Err(status) => Err(parse_status(
                ErrorContext::new(ServiceOperation("DeleteObject")),
                status,
            )),
        }
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
