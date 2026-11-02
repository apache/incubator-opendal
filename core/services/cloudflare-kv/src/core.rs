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

use http::Request;
use http::Response;
use http::header;
use http::request;
use opendal_core::raw::*;
use opendal_core::*;
use opendal_core::{Buffer, Result};
use serde_json::json;

use crate::model::CfKvMetadata;

#[derive(Debug, Clone)]
pub struct CloudflareKvCore {
    pub api_token: String,
    pub account_id: String,
    pub namespace_id: String,
    pub expiration_ttl: Option<Duration>,
    pub info: ServiceInfo,
    pub capability: Capability,
}

impl CloudflareKvCore {
    #[inline]
    async fn send(&self, ctx: &OperationContext, req: Request<Buffer>) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    fn sign(&self, req: request::Builder) -> request::Builder {
        req.header(header::AUTHORIZATION, &self.api_token)
    }

    fn url_prefix(&self) -> String {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/storage/kv/namespaces/{}",
            self.account_id, self.namespace_id
        );
        url
    }
}

impl CloudflareKvCore {
    pub async fn metadata(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let url = format!(
            "{}/metadata/{}",
            self.url_prefix(),
            percent_encode_path(path)
        );

        let req = Request::get(url);
        let req = self.sign(req);

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetMetadata"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn get(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let url = format!("{}/values/{}", self.url_prefix(), percent_encode_path(path));
        let req = Request::get(url);

        let req = self.sign(req);

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetValue"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn set(
        &self,
        ctx: &OperationContext,
        path: &str,
        value: Buffer,
        metadata: CfKvMetadata,
    ) -> Result<Response<Buffer>> {
        let url = format!("{}/values/{}", self.url_prefix(), percent_encode_path(path));

        let req = Request::put(url);
        let req = self.sign(req);
        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("WriteValue"));

        let mut multipart = Multipart::new()
            .part(FormDataPart::new("value").content(value))
            .part(
                FormDataPart::new("metadata")
                    .content(serde_json::to_string(&metadata).map_err(new_json_serialize_error)?),
            );

        if let Some(expiration_ttl) = self.expiration_ttl {
            multipart = multipart.part(
                FormDataPart::new("expiration_ttl").content(expiration_ttl.as_secs().to_string()),
            );
        }

        let req = multipart.apply(req)?;

        self.send(ctx, req).await
    }

    pub async fn delete(
        &self,
        ctx: &OperationContext,
        paths: &[String],
    ) -> Result<Response<Buffer>> {
        let url = format!("{}/bulk/delete", self.url_prefix());

        let req = Request::post(&url);

        let req = self.sign(req);
        let req_body = &json!(paths);
        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("BulkDelete"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Buffer::from(req_body.to_string()))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn list(
        &self,
        ctx: &OperationContext,
        prefix: &str,
        limit: Option<usize>,
        cursor: Option<String>,
    ) -> Result<Response<Buffer>> {
        let url = format!("{}/keys", self.url_prefix());
        let mut url = QueryPairsWriter::new(&url);
        if let Some(cursor) = cursor
            && !cursor.is_empty()
        {
            url = url.push("cursor", &cursor);
        }
        url = url.push("limit", &limit.unwrap_or(1000).to_string());
        url = url.push("prefix", &percent_encode_path(prefix));

        let req = Request::get(url.finish());

        let req = self.sign(req);
        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("ListKeys"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }
}

use bytes::Buf;
use http::StatusCode;
use serde_json::de;

use crate::model::CfKvError;
use crate::model::CfKvResponse;

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
pub(crate) fn parse_error(ctx: ErrorContext, resp: Response<Buffer>) -> Error {
    let (parts, body) = resp.into_parts();
    let bs = body.to_bytes();

    let (mut kind, mut retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        // Some services (like owncloud) return 403 while file locked.
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, true),
        // Allowing retry for resource locked.
        StatusCode::LOCKED => (ErrorKind::Unexpected, true),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, err) = de::from_reader::<_, CfKvResponse>(bs.clone().reader())
        .map(|err| (format!("{err:?}"), Some(err)))
        .unwrap_or_else(|_| (String::from_utf8_lossy(&bs).into_owned(), None));

    if let Some(err) = err {
        (kind, retryable) = parse_cfkv_error_code(err.errors).unwrap_or((kind, retryable));
    }

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

pub(crate) fn parse_cfkv_error_code(errors: Vec<CfKvError>) -> Option<(ErrorKind, bool)> {
    if errors.is_empty() {
        return None;
    }

    match errors[0].code {
        // The request is malformed: failed to decode id.
        7400 => Some((ErrorKind::Unexpected, false)),
        // no such column: Xxxx.
        7500 => Some((ErrorKind::NotFound, false)),
        // Authentication error.
        10000 => Some((ErrorKind::PermissionDenied, false)),
        _ => None,
    }
}
