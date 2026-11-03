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
use std::sync::Arc;

use asyncband::once::OnceCell;
use bytes::Buf;
use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use serde::Deserialize;

use opendal_core::raw::*;
use opendal_core::*;

/// Core of [seafile](https://www.seafile.com) services support.
#[derive(Clone)]
pub struct SeafileCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    /// The root of this core.
    pub root: String,
    /// The endpoint of this backend.
    pub endpoint: String,
    /// The username of this backend.
    pub username: String,
    /// The password id of this backend.
    pub password: String,
    /// The repo name of this backend.
    pub repo_name: String,

    /// Authentication and library information published after both requests succeed.
    pub auth_info: Arc<OnceCell<AuthInfo>>,
}

impl Debug for SeafileCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeafileCore")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .field("repo_name", &self.repo_name)
            .finish_non_exhaustive()
    }
}

impl SeafileCore {
    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    /// Initialize authentication and library information, retrying incomplete attempts.
    pub async fn get_auth_info(&self, ctx: &OperationContext) -> Result<AuthInfo> {
        let auth_info = self
            .auth_info
            .get_or_try_init(async || {
                let body = format!(
                    "username={}&password={}",
                    percent_encode_path(&self.username),
                    percent_encode_path(&self.password)
                );
                let req = Request::post(format!("{}/api2/auth-token/", self.endpoint))
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Buffer::from(Bytes::from(body)))
                    .map_err(new_request_build_error)?;

                let resp = ctx.http_transport().send(req).await?;
                let status = resp.status();

                let token = match status {
                    StatusCode::OK => {
                        let resp_body = resp.into_body();
                        let auth_response: AuthTokenResponse =
                            serde_json::from_reader(resp_body.reader())
                                .map_err(new_json_deserialize_error)?;
                        auth_response.token
                    }
                    _ => {
                        return Err(parse_error(
                            ErrorContext::new(ServiceOperation("AuthToken")),
                            resp,
                        ));
                    }
                };

                let url = format!("{}/api2/repos", self.endpoint);

                let req = Request::get(url)
                    .header(header::AUTHORIZATION, format!("Token {token}"))
                    .body(Buffer::new())
                    .map_err(new_request_build_error)?;

                let resp = ctx.http_transport().send(req).await?;

                let status = resp.status();

                let mut repo_id = String::new();
                match status {
                    StatusCode::OK => {
                        let resp_body = resp.into_body();
                        let list_library_response: Vec<ListLibraryResponse> =
                            serde_json::from_reader(resp_body.reader())
                                .map_err(new_json_deserialize_error)?;

                        for library in list_library_response {
                            if library.name == self.repo_name {
                                repo_id = library.id;
                                break;
                            }
                        }

                        // repo not found
                        if repo_id.is_empty() {
                            return Err(Error::new(
                                ErrorKind::NotFound,
                                format!("repo {} not found", self.repo_name),
                            ));
                        }
                    }
                    _ => {
                        return Err(parse_error(
                            ErrorContext::new(ServiceOperation("ListRepositories")),
                            resp,
                        ));
                    }
                }
                Ok(AuthInfo { repo_id, token })
            })
            .await?;
        Ok(auth_info.clone())
    }
}

impl SeafileCore {
    /// get upload url
    async fn get_upload_url(&self, ctx: &OperationContext) -> Result<String> {
        let auth_info = self.get_auth_info(ctx).await?;

        let req = Request::get(format!(
            "{}/api2/repos/{}/upload-link/",
            self.endpoint, auth_info.repo_id
        ));

        let req = req
            .header(header::AUTHORIZATION, format!("Token {}", auth_info.token))
            .extension(Operation::Write)
            .extension(ServiceOperation("GetUploadLink"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;
        let status = resp.status();

        match status {
            StatusCode::OK => {
                let resp_body = resp.into_body();
                let upload_url = serde_json::from_reader(resp_body.reader())
                    .map_err(new_json_deserialize_error)?;
                Ok(upload_url)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetUploadLink")),
                resp,
            )),
        }
    }

    pub async fn upload_file(
        &self,
        ctx: &OperationContext,
        path: &str,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let upload_url = self.get_upload_url(ctx).await?;

        let req = Request::post(upload_url)
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadFile"));

        let (filename, relative_path) = if path.ends_with('/') {
            ("", build_abs_path(&self.root, path))
        } else {
            let (filename, relative_path) = (get_basename(path), get_parent(path));
            (filename, build_abs_path(&self.root, relative_path))
        };

        let file_part = FormDataPart::new("file")
            .header(
                header::CONTENT_DISPOSITION,
                format!("form-data; name=\"file\"; filename=\"{filename}\"")
                    .parse()
                    .unwrap(),
            )
            .content(body);

        let multipart = Multipart::new()
            .part(FormDataPart::new("parent_dir").content("/"))
            .part(FormDataPart::new("relative_path").content(relative_path.clone()))
            .part(FormDataPart::new("replace").content("1"))
            .part(file_part);

        let req = multipart.apply(req)?;

        self.send(ctx, req).await
    }

    pub async fn create_dir(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        if path == "/" {
            return Ok(());
        }

        let path = build_rooted_abs_path(&self.root, path);
        let path = percent_encode_path(path.trim_end_matches('/'));

        let auth_info = self.get_auth_info(ctx).await?;
        let body = Buffer::from(Bytes::from_static(b"operation=mkdir&create_parents=true"));

        let req = Request::post(format!(
            "{}/api2/repos/{}/dir/?p={}",
            self.endpoint, auth_info.repo_id, path
        ));

        let req = req
            .header(header::AUTHORIZATION, format!("Token {}", auth_info.token))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("Mkdir"))
            .body(body)
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        match resp.status() {
            StatusCode::OK | StatusCode::CREATED => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Mkdir")),
                resp,
            )),
        }
    }

    /// get download
    async fn get_download_url(&self, ctx: &OperationContext, path: &str) -> Result<String> {
        let path = build_abs_path(&self.root, path);
        let path = percent_encode_path(&path);

        let auth_info = self.get_auth_info(ctx).await?;

        let req = Request::get(format!(
            "{}/api2/repos/{}/file/?p={}",
            self.endpoint, auth_info.repo_id, path
        ));

        let req = req
            .header(header::AUTHORIZATION, format!("Token {}", auth_info.token))
            .extension(Operation::Read)
            .extension(ServiceOperation("GetDownloadLink"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;
        let status = resp.status();

        match status {
            StatusCode::OK => {
                let resp_body = resp.into_body();
                let download_url = serde_json::from_reader(resp_body.reader())
                    .map_err(new_json_deserialize_error)?;

                Ok(download_url)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetDownloadLink")),
                resp,
            )),
        }
    }

    /// download file
    pub async fn download_file(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let download_url = self.get_download_url(ctx, path).await?;

        let req = Request::get(download_url);

        let req = req
            .header(header::RANGE, range.to_header())
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadFile"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    /// file detail
    pub async fn file_detail(&self, ctx: &OperationContext, path: &str) -> Result<FileDetail> {
        let path = build_abs_path(&self.root, path);
        let path = percent_encode_path(&path);

        let auth_info = self.get_auth_info(ctx).await?;

        let req = Request::get(format!(
            "{}/api2/repos/{}/file/detail/?p={}",
            self.endpoint, auth_info.repo_id, path
        ));

        let req = req
            .header(header::AUTHORIZATION, format!("Token {}", auth_info.token))
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetFileDetail"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;
        let status = resp.status();

        match status {
            StatusCode::OK => {
                let resp_body = resp.into_body();
                let file_detail: FileDetail = serde_json::from_reader(resp_body.reader())
                    .map_err(new_json_deserialize_error)?;
                Ok(file_detail)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetFileDetail")),
                resp,
            )),
        }
    }

    /// dir detail
    pub async fn dir_detail(&self, ctx: &OperationContext, path: &str) -> Result<DirDetail> {
        let path = build_abs_path(&self.root, path);
        let path = percent_encode_path(&path);

        let auth_info = self.get_auth_info(ctx).await?;

        let req = Request::get(format!(
            "{}/api/v2.1/repos/{}/dir/detail/?path={}",
            self.endpoint, auth_info.repo_id, path
        ));

        let req = req
            .header(header::AUTHORIZATION, format!("Token {}", auth_info.token))
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetDirDetail"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;
        let status = resp.status();

        match status {
            StatusCode::OK => {
                let resp_body = resp.into_body();
                let dir_detail: DirDetail = serde_json::from_reader(resp_body.reader())
                    .map_err(new_json_deserialize_error)?;
                Ok(dir_detail)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetDirDetail")),
                resp,
            )),
        }
    }

    /// delete file or dir
    pub async fn delete(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let path = build_abs_path(&self.root, path);
        let path = percent_encode_path(&path);

        let auth_info = self.get_auth_info(ctx).await?;

        let url = if path.ends_with('/') {
            format!(
                "{}/api2/repos/{}/dir/?p={}",
                self.endpoint, auth_info.repo_id, path
            )
        } else {
            format!(
                "{}/api2/repos/{}/file/?p={}",
                self.endpoint, auth_info.repo_id, path
            )
        };

        let req = Request::delete(url);

        let req = req
            .header(header::AUTHORIZATION, format!("Token {}", auth_info.token))
            .extension(Operation::Delete)
            .extension(ServiceOperation("Delete"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Delete")),
                resp,
            )),
        }
    }

    pub async fn list(&self, ctx: &OperationContext, path: &str) -> Result<ListResponse> {
        let rooted_abs_path = build_rooted_abs_path(&self.root, path);

        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!(
            "{}/api2/repos/{}/dir/?p={}",
            self.endpoint,
            auth_info.repo_id,
            percent_encode_path(&rooted_abs_path)
        );

        let req = Request::get(url);

        let req = req
            .header(header::AUTHORIZATION, format!("Token {}", auth_info.token))
            .extension(Operation::List)
            .extension(ServiceOperation("ListDir"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        match resp.status() {
            StatusCode::OK => {
                let resp_body = resp.into_body();
                let infos: Vec<Info> = serde_json::from_reader(resp_body.reader())
                    .map_err(new_json_deserialize_error)?;
                Ok(ListResponse {
                    infos: Some(infos),
                    rooted_abs_path,
                })
            }
            // return nothing when not exist
            StatusCode::NOT_FOUND => Ok(ListResponse {
                infos: None,
                rooted_abs_path,
            }),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("ListDir")),
                resp,
            )),
        }
    }
}

#[derive(Deserialize)]
pub struct AuthTokenResponse {
    pub token: String,
}

#[derive(Deserialize)]
pub struct FileDetail {
    pub last_modified: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
pub struct DirDetail {
    mtime: String,
}

pub fn parse_dir_detail(dir_detail: DirDetail) -> Result<Metadata> {
    let mut md = MetadataBuilder::dir();

    md.last_modified(dir_detail.mtime.parse::<Timestamp>()?);

    Ok(md.build())
}

pub fn parse_file_detail(file_detail: FileDetail) -> Result<Metadata> {
    let mut md = MetadataBuilder::file(file_detail.size);
    md.last_modified(file_detail.last_modified.parse::<Timestamp>()?);

    Ok(md.build())
}

#[derive(Clone)]
pub struct AuthInfo {
    /// The repo id of this auth info.
    pub repo_id: String,
    /// The token of this auth info,
    pub token: String,
}

#[derive(Deserialize)]
pub struct ListLibraryResponse {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct Info {
    #[serde(rename = "type")]
    pub type_field: String,
    pub mtime: i64,
    pub size: Option<u64>,
    pub name: String,
}

pub struct ListResponse {
    pub infos: Option<Vec<Info>>,
    pub rooted_abs_path: String,
}

/// the error response of seafile
#[derive(Default, Debug, Deserialize)]
#[allow(dead_code)]
struct SeafileError {
    error_msg: String,
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
        403 => (ErrorKind::PermissionDenied, false),
        404 => (ErrorKind::NotFound, false),
        // Seafile reports a transient backend failure as a 5xx, including its own 520;
        // retrying is the documented recovery, so mark these temporary.
        500 | 502 | 503 | 504 | 520 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, _seafile_err) = serde_json::from_reader::<_, SeafileError>(bs.clone().reader())
        .map(|seafile_err| (format!("{seafile_err:?}"), Some(seafile_err)))
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
    use std::collections::VecDeque;
    use std::future::Future;
    use std::task::Context;
    use std::task::Waker;

    use asyncband::mutex::Mutex;
    use asyncband::semaphore::Semaphore;
    use http::StatusCode;

    use super::*;

    #[derive(Clone)]
    struct AuthTransport {
        responses: Arc<Mutex<VecDeque<(&'static str, &'static str)>>>,
        repositories: Arc<Semaphore>,
    }

    impl AuthTransport {
        fn new(responses: &[(&'static str, &'static str)]) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses.iter().copied().collect())),
                repositories: Arc::new(Semaphore::new(0)),
            }
        }
    }

    impl HttpTransport for AuthTransport {
        async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
            let (path, body) = self.responses.lock().await.pop_front().unwrap();
            assert_eq!(req.uri().path(), path);
            if path == "/api2/repos" {
                drop(self.repositories.acquire(1).await);
            }
            Ok(Response::builder()
                .body(HttpBody::new(
                    futures::stream::iter([Ok(Buffer::from(Bytes::from_static(body.as_bytes())))]),
                    Some(body.len() as u64),
                ))
                .unwrap())
        }
    }

    fn auth_core(transport: AuthTransport) -> (SeafileCore, OperationContext) {
        let core = SeafileCore {
            info: ServiceInfo::new("seafile", "/", ""),
            capability: Capability::default(),
            root: "/".to_string(),
            endpoint: "http://example.com".to_string(),
            username: "user".to_string(),
            password: "password".to_string(),
            repo_name: "test".to_string(),
            auth_info: Arc::new(OnceCell::new()),
        };
        let ctx = OperationContext::new().with_http_transport(HttpTransporter::new(transport));
        (core, ctx)
    }

    #[tokio::test]
    async fn test_auth_info_retries_after_missing_repository() {
        let transport = AuthTransport::new(&[
            ("/api2/auth-token/", r#"{"token":"first"}"#),
            ("/api2/repos", "[]"),
            ("/api2/auth-token/", r#"{"token":"second"}"#),
            ("/api2/repos", r#"[{"name":"test","id":"repo-id"}]"#),
        ]);
        transport.repositories.release(1);
        let (core, ctx) = auth_core(transport.clone());

        let err = core.get_auth_info(&ctx).await.err().unwrap();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        let auth = core.get_auth_info(&ctx).await.unwrap();
        assert_eq!(auth.repo_id, "repo-id");
        assert_eq!(auth.token, "second");
        assert!(transport.responses.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_auth_info_retries_after_initialization_is_cancelled() {
        let transport = AuthTransport::new(&[
            ("/api2/auth-token/", r#"{"token":"first"}"#),
            ("/api2/repos", r#"[{"name":"test","id":"repo-id"}]"#),
            ("/api2/auth-token/", r#"{"token":"second"}"#),
            ("/api2/repos", r#"[{"name":"test","id":"repo-id"}]"#),
        ]);
        let (core, ctx) = auth_core(transport.clone());
        let mut first = Box::pin(core.get_auth_info(&ctx));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(first.as_mut().poll(&mut cx).is_pending());
        drop(first);

        transport.repositories.release(1);
        let auth = core.get_auth_info(&ctx).await.unwrap();
        assert_eq!(auth.repo_id, "repo-id");
        assert_eq!(auth.token, "second");
        assert!(transport.responses.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_auth_info_shares_complete_initialization() {
        let transport = AuthTransport::new(&[
            ("/api2/auth-token/", r#"{"token":"shared"}"#),
            ("/api2/repos", r#"[{"name":"test","id":"repo-id"}]"#),
        ]);
        let (core, ctx) = auth_core(transport.clone());
        let mut first = Box::pin(core.get_auth_info(&ctx));
        let mut second = Box::pin(core.get_auth_info(&ctx));
        let mut cancelled = Box::pin(core.get_auth_info(&ctx));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(first.as_mut().poll(&mut cx).is_pending());
        assert!(second.as_mut().poll(&mut cx).is_pending());
        assert!(cancelled.as_mut().poll(&mut cx).is_pending());
        drop(cancelled);

        transport.repositories.release(1);
        let (first, second) = tokio::join!(first, second);
        for auth in [first, second, core.get_auth_info(&ctx).await] {
            let auth = auth.unwrap();
            assert_eq!(auth.repo_id, "repo-id");
            assert_eq!(auth.token, "shared");
        }
        assert!(transport.responses.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_parse_error() {
        let err_res = vec![
            (
                r#"{"error_msg": "Permission denied"}"#,
                ErrorKind::PermissionDenied,
                StatusCode::FORBIDDEN,
            ),
            (
                r#"{"error_msg": "Folder /e982e75a-fead-487c-9f41-63094d9bf0de/a9d867b9-778d-4612-b674-47e674c14c28/ not found."}"#,
                ErrorKind::NotFound,
                StatusCode::NOT_FOUND,
            ),
        ];

        for res in err_res {
            let bs = bytes::Bytes::from(res.0);
            let body = Buffer::from(bs);
            let resp = Response::builder().status(res.2).body(body).unwrap();

            let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);

            assert_eq!(err.kind(), res.1);
        }
    }
}
