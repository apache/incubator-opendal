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

use std::collections::HashMap;
use std::fmt::Debug;

use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use http::request;
use serde::Deserialize;
use serde::Serialize;

use opendal_core::raw::*;
use opendal_core::*;

/// Core of the [GitHub Gist](https://docs.github.com/en/rest/gists/gists) service.
#[derive(Clone)]
pub struct GistCore {
    pub info: ServiceInfo,
    /// GitHub access_token.
    pub token: Option<String>,
    /// Identifier of the gist this core operates on.
    pub gist_id: String,
}

impl Debug for GistCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GistCore")
            .field("gist_id", &self.gist_id)
            .finish_non_exhaustive()
    }
}

impl GistCore {
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    fn sign(
        &self,
        req: request::Builder,
        op: Operation,
        service_operation: ServiceOperation,
    ) -> Result<request::Builder> {
        let mut req = req
            .extension(op)
            .extension(service_operation)
            .header(header::USER_AGENT, format!("opendal-{VERSION}"))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header(header::ACCEPT, "application/vnd.github+json");

        if let Some(token) = &self.token {
            req = req.header(
                header::AUTHORIZATION,
                format_authorization_by_bearer(token)?,
            );
        }

        Ok(req)
    }

    fn gist_url(&self) -> String {
        format!("https://api.github.com/gists/{}", self.gist_id)
    }

    /// Fetch the gist and return its description.
    ///
    /// Returns `None` when the gist does not exist or is not visible with the
    /// current credential.
    pub async fn get_gist(&self, ctx: &OperationContext, op: Operation) -> Result<Option<Gist>> {
        let req = self.sign(
            Request::get(self.gist_url()),
            op,
            ServiceOperation("GetGist"),
        )?;
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        match resp.status() {
            StatusCode::OK => {
                let body = resp.into_body();
                let gist: Gist =
                    serde_json::from_slice(&body.to_bytes()).map_err(new_json_deserialize_error)?;
                Ok(Some(gist))
            }
            StatusCode::NOT_FOUND => Ok(None),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetGist")),
                resp,
            )),
        }
    }

    /// Fetch a single file of the gist.
    ///
    /// Returns `None` when the gist or the file does not exist.
    pub async fn get_file(
        &self,
        ctx: &OperationContext,
        name: &str,
        op: Operation,
    ) -> Result<Option<GistFile>> {
        let Some(gist) = self.get_gist(ctx, op).await? else {
            return Ok(None);
        };

        Ok(gist
            .files
            .into_iter()
            .find(|(key, _)| key == name)
            .map(|(_, file)| file))
    }

    /// Update, create, or delete files of the gist.
    ///
    /// [`UpdateFilesRequest`] carries the whole change set, because the Gist API
    /// applies every file change of one edit in a single request.
    pub async fn update_files(
        &self,
        ctx: &OperationContext,
        op: Operation,
        files: UpdateFilesRequest,
    ) -> Result<Response<Buffer>> {
        let req = self.sign(
            Request::patch(self.gist_url()),
            op,
            ServiceOperation("UpdateGist"),
        )?;

        let req = req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Buffer::from(
                serde_json::to_vec(&files).map_err(new_json_serialize_error)?,
            ))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    /// Fetch the raw content of a file.
    ///
    /// The token is deliberately not forwarded: `raw_url` points at
    /// `gist.githubusercontent.com`, which serves the content by URL alone and
    /// takes no GitHub credential.
    pub async fn fetch_raw(
        &self,
        ctx: &OperationContext,
        url: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let req = Request::get(url)
            .extension(Operation::Read)
            .extension(ServiceOperation("GetGistRawFile"))
            .header(header::USER_AGENT, format!("opendal-{VERSION}"))
            .header(header::RANGE, range.to_header())
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }
}

/// Metadata of a gist file, as returned by this service.
pub fn parse_metadata(file: &GistFile) -> Metadata {
    let mut builder = MetadataBuilder::file(file.size);

    if let Some(content_type) = &file.content_type {
        builder.content_type(content_type);
    }

    builder.build()
}

/// A gist as returned by the GitHub API.
#[derive(Default, Debug, Clone, Deserialize)]
pub struct Gist {
    #[serde(default)]
    pub files: HashMap<String, GistFile>,
}

/// A single file of a gist.
///
/// The file name is the key in [`Gist::files`], so the response's own `filename`
/// field carries no extra information.
#[derive(Default, Debug, Clone, Deserialize)]
pub struct GistFile {
    /// Media type of the file content.
    #[serde(default, rename = "type")]
    pub content_type: Option<String>,
    /// Size of the file in bytes.
    #[serde(default)]
    pub size: u64,
    /// URL returning the raw content of this file.
    #[serde(default)]
    pub raw_url: String,
    /// Whether the API truncated [`GistFile::content`] because the file is too large.
    #[serde(default)]
    pub truncated: bool,
    /// Inline content of the file.
    ///
    /// The API returns at most 1 MiB per file and omits the field on endpoints
    /// that do not embed content.
    #[serde(default)]
    pub content: Option<String>,
}

/// Change set sent to the Gist update endpoint.
#[derive(Default, Debug, Clone, Serialize)]
pub struct UpdateFilesRequest {
    /// Mapping of file name to its change, where `None` deletes the file.
    pub files: HashMap<String, Option<UpdateFileRequest>>,
}

/// Change applied to one file of a gist.
#[derive(Default, Debug, Clone, Serialize)]
pub struct UpdateFileRequest {
    /// New content of the file.
    pub content: String,
}

#[derive(Default, Debug, Deserialize)]
#[allow(dead_code)]
struct GistErrorBody {
    message: String,
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

    // GitHub reports an exhausted quota as 403 (or 429 for secondary limits)
    // together with a zero remaining count, which is retryable in a way a plain
    // forbidden response is not.
    let rate_limited = parts
        .headers
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == "0");

    let (kind, retryable) = match parts.status.as_u16() {
        403 | 429 if rate_limited => (ErrorKind::RateLimited, true),
        401 | 403 => (ErrorKind::PermissionDenied, false),
        404 => (ErrorKind::NotFound, false),
        416 => (ErrorKind::RangeNotSatisfied, false),
        429 => (ErrorKind::RateLimited, true),
        500 | 502 | 503 | 504 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = serde_json::from_slice::<GistErrorBody>(&bs)
        .map(|body| body.message)
        .unwrap_or_else(|_| String::from_utf8_lossy(&bs).into_owned());

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error_response(status: StatusCode, rate_limited: bool) -> Response<Buffer> {
        let body = Buffer::from(
            r#"{"message":"Not Found","documentation_url":"https://docs.github.com/rest/gists"}"#,
        );

        let mut builder = Response::builder().status(status);
        if rate_limited {
            builder = builder.header("x-ratelimit-remaining", "0");
        }

        builder.body(body).expect("response must build")
    }

    #[test]
    fn test_parse_error() {
        let cases = [
            (StatusCode::NOT_FOUND, false, ErrorKind::NotFound),
            (StatusCode::UNAUTHORIZED, false, ErrorKind::PermissionDenied),
            (StatusCode::FORBIDDEN, false, ErrorKind::PermissionDenied),
            (StatusCode::FORBIDDEN, true, ErrorKind::RateLimited),
            (
                StatusCode::RANGE_NOT_SATISFIABLE,
                false,
                ErrorKind::RangeNotSatisfied,
            ),
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                false,
                ErrorKind::Unexpected,
            ),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                false,
                ErrorKind::Unexpected,
            ),
        ];

        for (status, rate_limited, expected) in cases {
            let err = parse_error(
                ErrorContext::new(ServiceOperation("Test")),
                error_response(status, rate_limited),
            );

            assert_eq!(err.kind(), expected, "status {status}");
        }
    }

    #[test]
    fn test_parse_error_marks_rate_limit_temporary() {
        let err = parse_error(
            ErrorContext::new(ServiceOperation("Test")),
            error_response(StatusCode::FORBIDDEN, true),
        );

        assert!(err.is_temporary());
    }

    #[test]
    fn test_parse_error_keeps_api_message() {
        let err = parse_error(
            ErrorContext::new(ServiceOperation("Test")),
            error_response(StatusCode::NOT_FOUND, false),
        );

        assert_eq!(err.message(), "Not Found");
    }
}
