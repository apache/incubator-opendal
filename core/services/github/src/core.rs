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
use bytes::Buf;
use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use http::request;
use serde::Deserialize;
use serde::Serialize;

use opendal_core::raw::*;
use opendal_core::*;

/// Core of [github contents](https://docs.github.com/en/rest/repos/contents?apiVersion=2022-11-28#create-or-update-file-contents) services support.
#[derive(Clone)]
pub struct GithubCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    /// The root of this core.
    pub root: String,
    /// Github access_token.
    pub token: Option<String>,
    /// Github repo owner.
    pub owner: String,
    /// Github repo name.
    pub repo: String,
}

impl Debug for GithubCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubCore")
            .field("root", &self.root)
            .field("owner", &self.owner)
            .field("repo", &self.repo)
            .finish_non_exhaustive()
    }
}

impl GithubCore {
    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    pub fn sign(&self, req: request::Builder) -> Result<request::Builder> {
        let mut req = req
            .header(header::USER_AGENT, format!("opendal-{VERSION}"))
            .header("X-GitHub-Api-Version", "2022-11-28");

        // Github access_token is optional.
        if let Some(token) = &self.token {
            req = req.header(
                header::AUTHORIZATION,
                format_authorization_by_bearer(token)?,
            )
        }

        Ok(req)
    }
}

impl GithubCore {
    pub async fn get_file_sha(&self, ctx: &OperationContext, path: &str) -> Result<Option<String>> {
        // if the token is not set, we should not try to get the sha of the file.
        if self.token.is_none() {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "Github access_token is not set",
            ));
        }

        let resp = self.stat(ctx, path).await?;

        match resp.status() {
            StatusCode::OK => {
                let body = resp.into_body();
                let resp: Entry =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

                Ok(Some(resp.sha))
            }
            StatusCode::NOT_FOUND => Ok(None),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetRepositoryContent")),
                resp,
            )),
        }
    }

    pub async fn stat(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.owner,
            self.repo,
            percent_encode_path(&path)
        );

        let req = Request::get(url);

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetRepositoryContent"));

        let req = self.sign(req)?;

        let req = req
            .header("Accept", "application/vnd.github.object+json")
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn get(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.owner,
            self.repo,
            percent_encode_path(&path)
        );

        let req = Request::get(url);

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetRepositoryContent"));

        let req = self.sign(req)?;

        let req = req
            .header(header::ACCEPT, "application/vnd.github.raw+json")
            .header(header::RANGE, range.to_header())
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        bs: Buffer,
    ) -> Result<Response<Buffer>> {
        let sha = self.get_file_sha(ctx, path).await?;

        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.owner,
            self.repo,
            percent_encode_path(&path)
        );

        let req = Request::put(url);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CreateOrUpdateFileContents"));

        let req = self.sign(req)?;

        let mut req_body = CreateOrUpdateContentsRequest {
            message: format!("Write {} at {} via opendal", path, Timestamp::now()),
            content: base64::engine::general_purpose::STANDARD.encode(bs.to_bytes()),
            sha: None,
        };

        if let Some(sha) = sha {
            req_body.sha = Some(sha);
        }

        let req_body = serde_json::to_vec(&req_body).map_err(new_json_serialize_error)?;

        let req = req
            .header("Accept", "application/vnd.github+json")
            .body(Buffer::from(req_body))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn delete(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        // If path is a directory, we should delete path/.gitkeep
        let formatted_path = format!("{path}.gitkeep");
        let p = if path.ends_with('/') {
            formatted_path.as_str()
        } else {
            path
        };

        let Some(sha) = self.get_file_sha(ctx, p).await? else {
            return Ok(());
        };

        let path = build_abs_path(&self.root, p);

        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.owner,
            self.repo,
            percent_encode_path(&path)
        );

        let req = Request::delete(url);

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteFile"));

        let req = self.sign(req)?;

        let req_body = DeleteContentsRequest {
            message: format!("Delete {} at {} via opendal", path, Timestamp::now()),
            sha,
        };

        let req_body = serde_json::to_vec(&req_body).map_err(new_json_serialize_error)?;

        let req = req
            .header("Accept", "application/vnd.github.object+json")
            .body(Buffer::from(Bytes::from(req_body)))
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        match resp.status() {
            StatusCode::OK => Ok(()),
            StatusCode::NOT_FOUND => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteFile")),
                resp,
            )),
        }
    }

    pub async fn list(&self, ctx: &OperationContext, path: &str) -> Result<ListResponse> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.owner,
            self.repo,
            percent_encode_path(&path)
        );

        let req = Request::get(url);

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("GetRepositoryContent"));

        let req = self.sign(req)?;

        let req = req
            .header("Accept", "application/vnd.github.object+json")
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        match resp.status() {
            StatusCode::OK => {
                let body = resp.into_body();
                let resp: ListResponse =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

                Ok(resp)
            }
            StatusCode::NOT_FOUND => Ok(ListResponse::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetRepositoryContent")),
                resp,
            )),
        }
    }

    /// We use git_url to call github's Tree based API.
    pub async fn list_with_recursive(
        &self,
        ctx: &OperationContext,
        git_url: &str,
    ) -> Result<Vec<Tree>> {
        let url = format!("{git_url}?recursive=true");

        let req = Request::get(url);

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("GetTree"));

        let req = self.sign(req)?;

        let req = req
            .header("Accept", "application/vnd.github.object+json")
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        match resp.status() {
            StatusCode::OK => {
                let body = resp.into_body();
                let resp: ListTreeResponse =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

                Ok(resp.tree)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetTree")),
                resp,
            )),
        }
    }
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct CreateOrUpdateContentsRequest {
    pub message: String,
    pub content: String,
    pub sha: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct DeleteContentsRequest {
    pub message: String,
    pub sha: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ListTreeResponse {
    pub tree: Vec<Tree>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct Tree {
    pub path: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub size: Option<u64>,
    pub sha: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ListResponse {
    pub git_url: String,
    pub entries: Vec<Entry>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct Entry {
    pub path: String,
    pub sha: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub type_field: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct ContentResponse {
    pub content: Entry,
}

#[derive(Default, Debug, Deserialize)]
#[allow(dead_code)]
struct GithubError {
    error: GithubSubError,
}

#[derive(Default, Debug, Deserialize)]
#[allow(dead_code)]
struct GithubSubError {
    message: String,
    documentation_url: String,
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

    let (kind, retryable) = match parts.status.as_u16() {
        401 | 403 => (ErrorKind::PermissionDenied, false),
        404 => (ErrorKind::NotFound, false),
        304 | 412 => (ErrorKind::ConditionNotMatch, false),
        // https://github.com/apache/opendal/issues/4146
        // https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/423
        // We should retry it when we get 423 error.
        423 => (ErrorKind::RateLimited, true),
        // Service like Upyun could return 499 error with a message like:
        // Client Disconnect, we should retry it.
        499 => (ErrorKind::Unexpected, true),
        500 | 502 | 503 | 504 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, _github_content_err) =
        serde_json::from_reader::<_, GithubError>(bs.clone().reader())
            .map(|github_content_err| (format!("{github_content_err:?}"), Some(github_content_err)))
            .unwrap_or_else(|_| (String::from_utf8_lossy(&bs).into_owned(), None));

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
    use http::StatusCode;

    use super::*;

    #[tokio::test]
    async fn test_parse_error() {
        let err_res = vec![(
            r#"{
                "message": "Not Found",
                "documentation_url": "https://docs.github.com/rest/repos/contents#get-repository-content"
            }"#,
            ErrorKind::NotFound,
            StatusCode::NOT_FOUND,
        )];

        for res in err_res {
            let bs = bytes::Bytes::from(res.0);
            let body = Buffer::from(bs);
            let resp = Response::builder().status(res.2).body(body).unwrap();

            let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);

            assert_eq!(err.kind(), res.1);
        }
    }
}
