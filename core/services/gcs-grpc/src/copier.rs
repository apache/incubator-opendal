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

use crate::core::{ErrorContext, GcsGrpcCore, parse_generation, parse_object, parse_status};
use crate::generated::google::storage::v2::RewriteObjectRequest;

pub(super) fn new_gcs_grpc_copier(
    core: Arc<GcsGrpcCore>,
    ctx: OperationContext,
    from: &str,
    to: &str,
    args: OpCopy,
) -> oio::OneShotCopier {
    let from = from.to_string();
    let to = to.to_string();
    oio::OneShotCopier::new_with(move || {
        let core = core.clone();
        let ctx = ctx.clone();
        let from = from.clone();
        let to = to.clone();
        let args = args.clone();
        async move { copy_object(core, ctx, from, to, args).await }
    })
}

async fn copy_object(
    core: Arc<GcsGrpcCore>,
    ctx: OperationContext,
    from: String,
    to: String,
    args: OpCopy,
) -> Result<Metadata> {
    let mut request = RewriteObjectRequest {
        destination_name: core.object_name(&to),
        destination_bucket: core.bucket_resource(),
        source_bucket: core.bucket_resource(),
        source_object: core.object_name(&from),
        source_generation: parse_generation(args.source_version())?,
        if_generation_match: args.if_not_exists().then_some(0),
        ..Default::default()
    };
    let bucket = core.bucket_resource();
    loop {
        let tonic_request = core
            .request(
                &ctx,
                request.clone(),
                &[("source_bucket", &bucket), ("bucket", &bucket)],
            )
            .await?;
        let response = core
            .client()
            .rewrite_object(tonic_request)
            .await
            .map_err(|status| {
                parse_status(
                    ErrorContext::new(ServiceOperation("RewriteObject"))
                        .with_if_not_exists(args.if_not_exists()),
                    status,
                )
            })?
            .into_inner();
        if response.done {
            return response.resource.as_ref().map(parse_object).ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "GCS rewrite completed without object metadata",
                )
            });
        }
        if response.rewrite_token.is_empty() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "GCS rewrite requires continuation but returned no token",
            ));
        }
        request.rewrite_token = response.rewrite_token;
    }
}
