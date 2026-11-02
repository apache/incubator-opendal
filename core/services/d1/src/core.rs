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
use http::StatusCode;
use http::header;
use serde_json::Value;

use super::model::*;
use opendal_core::raw::*;
use opendal_core::*;

#[derive(Clone)]
pub struct D1Core {
    pub authorization: Option<String>,
    pub account_id: String,
    pub database_id: String,

    pub table: String,
    pub key_field: String,
    pub value_field: String,
}

impl Debug for D1Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("D1Core")
            .field("table", &self.table)
            .field("key_field", &self.key_field)
            .field("value_field", &self.value_field)
            .finish_non_exhaustive()
    }
}

const CLOUDFLARE_API_BASE_URL: &str = "https://api.cloudflare.com/client/v4";

impl D1Core {
    fn create_d1_query_request(
        &self,
        sql: &str,
        params: Vec<Value>,
        op: Operation,
        service_operation: &'static str,
    ) -> Result<Request<Buffer>> {
        let p = format!(
            "/accounts/{}/d1/database/{}/query",
            self.account_id, self.database_id
        );
        let url: String = format!("{}{}", CLOUDFLARE_API_BASE_URL, percent_encode_path(&p));

        let mut req = Request::post(&url);
        if let Some(auth) = &self.authorization {
            req = req.header(header::AUTHORIZATION, auth);
        }
        req = req.header(header::CONTENT_TYPE, "application/json");

        let json = serde_json::json!({
            "sql": sql,
            "params": params,
        });

        let body = serde_json::to_vec(&json).map_err(new_json_serialize_error)?;
        req.extension(op)
            .extension(ServiceOperation(service_operation))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)
    }

    pub async fn get(&self, ctx: &OperationContext, path: &str) -> Result<Option<Buffer>> {
        // d1 follows SQLite SQL syntax, we use `"` for identifier quote. A quoted identifier is case-sensitive and
        // can contain special characters.
        // Read more https://www.sqlite.org/quirks.html#double_quoted_string_literals
        //
        // We uses identifier quote for trusted table and field configuration to ensure correctness.
        let query = format!(
            r#"SELECT "{}" FROM "{}" WHERE "{}" = ? LIMIT 1"#,
            self.value_field, self.table, self.key_field
        );
        let req =
            self.create_d1_query_request(&query, vec![path.into()], Operation::Read, "Get")?;

        let resp = ctx.http_transport().send(req).await?;
        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                let body = resp.into_body();
                let bs = body.to_bytes();
                let d1_response = D1Response::parse(&bs)?;
                Ok(d1_response.get_result(&self.value_field))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Query")),
                resp,
            )),
        }
    }

    pub async fn get_length(&self, ctx: &OperationContext, path: &str) -> Result<Option<usize>> {
        let query = format!(
            r#"SELECT LENGTH(CAST("{}" AS BLOB)) AS "content_length" FROM "{}" WHERE "{}" = ? LIMIT 1"#,
            self.value_field, self.table, self.key_field
        );
        let req =
            self.create_d1_query_request(&query, vec![path.into()], Operation::Stat, "Stat")?;

        let resp = ctx.http_transport().send(req).await?;
        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                let body = resp.into_body();
                let bs = body.to_bytes();
                let d1_response = D1Response::parse(&bs)?;
                d1_response.get_usize_result("content_length")
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Query")),
                resp,
            )),
        }
    }

    pub async fn set(&self, ctx: &OperationContext, path: &str, value: Buffer) -> Result<()> {
        let table = &self.table;
        let key_field = &self.key_field;
        let value_field = &self.value_field;
        let query = format!(
            r#"INSERT INTO "{table}" ("{key_field}", "{value_field}")
                VALUES (?, ?)
                ON CONFLICT ("{key_field}")
                    DO UPDATE SET "{value_field}" = EXCLUDED."{value_field}""#,
        );

        let params = vec![path.into(), value.to_vec().into()];
        let req = self.create_d1_query_request(&query, params, Operation::Write, "Set")?;

        let resp = ctx.http_transport().send(req).await?;
        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Query")),
                resp,
            )),
        }
    }

    pub async fn delete(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let query = format!(
            r#"DELETE FROM "{}" WHERE "{}" = ?"#,
            self.table, self.key_field
        );
        let req =
            self.create_d1_query_request(&query, vec![path.into()], Operation::Delete, "Delete")?;

        let resp = ctx.http_transport().send(req).await?;
        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Query")),
                resp,
            )),
        }
    }
}

use bytes::Buf;
use http::Response;
use serde_json::de;

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

    let (message, d1_err) = de::from_reader::<_, D1Response>(bs.clone().reader())
        .map(|d1_err| (format!("{d1_err:?}"), Some(d1_err)))
        .unwrap_or_else(|_| (String::from_utf8_lossy(&bs).into_owned(), None));

    if let Some(d1_err) = d1_err {
        (kind, retryable) = parse_d1_error_code(d1_err.errors).unwrap_or((kind, retryable));
    }

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

pub fn parse_d1_error_code(errors: Vec<D1Error>) -> Option<(ErrorKind, bool)> {
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
