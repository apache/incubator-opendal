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

use std::fmt::Debug;

use http::Request;
use http::Response;
use http::header;

use opendal_core::raw::*;
use opendal_core::*;

pub struct VercelArtifactsCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub(crate) access_token: String,
    pub(crate) endpoint: String,
    pub(crate) query_string: String,
}

impl Debug for VercelArtifactsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VercelArtifactsCore")
            .finish_non_exhaustive()
    }
}

impl VercelArtifactsCore {
    pub(crate) async fn vercel_artifacts_get(
        &self,
        ctx: &OperationContext,
        hash: &str,
        range: BytesRange,
        _: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let url: String = format!(
            "{}/v8/artifacts/{}{}",
            self.endpoint,
            percent_encode_path(hash),
            self.query_string
        );

        let mut req = Request::get(&url);

        if !range.is_full() {
            req = req.header(header::RANGE, range.to_header());
        }

        let auth_header_content = format!("Bearer {}", self.access_token);
        req = req.header(header::AUTHORIZATION, auth_header_content);

        req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadArtifact"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub(crate) async fn vercel_artifacts_put(
        &self,
        ctx: &OperationContext,
        hash: &str,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let url = format!(
            "{}/v8/artifacts/{}{}",
            self.endpoint,
            percent_encode_path(hash),
            self.query_string
        );

        let mut req = Request::put(&url);

        let auth_header_content = format!("Bearer {}", self.access_token);
        req = req.header(header::CONTENT_TYPE, "application/octet-stream");
        req = req.header(header::AUTHORIZATION, auth_header_content);
        req = req.header(header::CONTENT_LENGTH, size);

        req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadArtifact"));

        let req = req.body(body).map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub(crate) async fn vercel_artifacts_stat(
        &self,
        ctx: &OperationContext,
        hash: &str,
    ) -> Result<Response<Buffer>> {
        let url = format!(
            "{}/v8/artifacts/{}{}",
            self.endpoint,
            percent_encode_path(hash),
            self.query_string
        );

        let mut req = Request::head(&url);

        let auth_header_content = format!("Bearer {}", self.access_token);
        req = req.header(header::AUTHORIZATION, auth_header_content);
        req = req.header(header::CONTENT_LENGTH, 0);

        req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("ArtifactExists"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }
}

use http::StatusCode;

/// Context needed to classify an error from this service.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrorContext {
    service_operation: ServiceOperation,
}

impl ErrorContext {
    pub(crate) const fn new(service_operation: ServiceOperation) -> Self {
        Self { service_operation }
    }
}

/// Parse an error response using its service request context.
pub(crate) fn parse_error(ctx: ErrorContext, response: Response<Buffer>) -> Error {
    let (parts, body) = response.into_parts();
    let bs = body.to_bytes();

    let (kind, retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = String::from_utf8_lossy(&bs);

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}
