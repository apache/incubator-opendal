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

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use serde::Deserialize;
use serde_json::json;

use opendal_core::raw::*;
use opendal_core::*;

pub struct DbfsCore {
    pub root: String,
    pub endpoint: String,
    pub token: String,
}

impl Debug for DbfsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbfsCore")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl DbfsCore {
    pub async fn dbfs_create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let url = format!("{}/api/2.0/dbfs/mkdirs", self.endpoint);
        let mut req = Request::post(&url);

        let auth_header_content = format!("Bearer {}", self.token);
        req = req.header(header::AUTHORIZATION, auth_header_content);

        let p = build_rooted_abs_path(&self.root, path)
            .trim_end_matches('/')
            .to_string();

        let req_body = &json!({
            "path": p,
        });
        let body = Buffer::from(Bytes::from(req_body.to_string()));

        let req = req
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("Mkdirs"))
            .body(body)
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn dbfs_delete(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let url = format!("{}/api/2.0/dbfs/delete", self.endpoint);
        let mut req = Request::post(&url);

        let auth_header_content = format!("Bearer {}", self.token);
        req = req.header(header::AUTHORIZATION, auth_header_content);

        let p = build_rooted_abs_path(&self.root, path)
            .trim_end_matches('/')
            .to_string();

        let request_body = &json!({
            "path": p,
            // TODO: support recursive toggle, should we add a new field in OpDelete?
            "recursive": true,
        });

        let body = Buffer::from(Bytes::from(request_body.to_string()));

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("Delete"))
            .body(body)
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn dbfs_rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let source = build_rooted_abs_path(&self.root, from);
        let target = build_rooted_abs_path(&self.root, to);

        let url = format!("{}/api/2.0/dbfs/move", self.endpoint);
        let mut req = Request::post(&url);

        let auth_header_content = format!("Bearer {}", self.token);
        req = req.header(header::AUTHORIZATION, auth_header_content);

        let req_body = &json!({
            "source_path": source,
            "destination_path": target,
        });

        let body = Buffer::from(Bytes::from(req_body.to_string()));

        let req = req
            .extension(Operation::Rename)
            .extension(ServiceOperation("Move"))
            .body(body)
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn dbfs_list(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path)
            .trim_end_matches('/')
            .to_string();

        let url = format!(
            "{}/api/2.0/dbfs/list?path={}",
            self.endpoint,
            percent_encode_path(&p)
        );
        let mut req = Request::get(&url);

        let auth_header_content = format!("Bearer {}", self.token);
        req = req.header(header::AUTHORIZATION, auth_header_content);

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("List"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub fn dbfs_create_file_request(&self, path: &str, body: Bytes) -> Result<Request<Buffer>> {
        let url = format!("{}/api/2.0/dbfs/put", self.endpoint);

        let contents = BASE64_STANDARD.encode(body);
        let mut req = Request::post(&url);

        let auth_header_content = format!("Bearer {}", self.token);
        req = req.header(header::AUTHORIZATION, auth_header_content);

        let req_body = &json!({
            "path": build_rooted_abs_path(&self.root, path),
            "contents": contents,
            "overwrite": true,
        });

        let body = Buffer::from(Bytes::from(req_body.to_string()));

        req.extension(Operation::Write)
            .extension(ServiceOperation("Put"))
            .body(body)
            .map_err(new_request_build_error)
    }

    pub async fn dbfs_get_status(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path)
            .trim_end_matches('/')
            .to_string();

        let url = format!(
            "{}/api/2.0/dbfs/get-status?path={}",
            self.endpoint,
            percent_encode_path(&p)
        );

        let mut req = Request::get(&url);

        let auth_header_content = format!("Bearer {}", self.token);
        req = req.header(header::AUTHORIZATION, auth_header_content);

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetStatus"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn dbfs_ensure_parent_path(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let resp = self.dbfs_get_status(ctx, path).await?;

        match resp.status() {
            StatusCode::OK => return Ok(()),
            StatusCode::NOT_FOUND => {
                self.dbfs_create_dir(ctx, path).await?;
            }
            _ => {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("GetStatus")),
                    resp,
                ));
            }
        }
        Ok(())
    }
}

/// DbfsError is the error returned by DBFS service.
#[derive(Default, Deserialize)]
struct DbfsError {
    error_code: String,
    message: String,
}

impl Debug for DbfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbfsError")
            .field("error_code", &self.error_code)
            // replace `\n` to ` ` for better reading.
            .field("message", &self.message.replace('\n', " "))
            .finish()
    }
}

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

    let (kind, retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::PRECONDITION_FAILED => (ErrorKind::ConditionNotMatch, false),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = match serde_json::from_slice::<DbfsError>(&bs) {
        Ok(dbfs_error) => format!("{:?}", dbfs_error.message),
        Err(_) => String::from_utf8_lossy(&bs).into_owned(),
    };

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}
