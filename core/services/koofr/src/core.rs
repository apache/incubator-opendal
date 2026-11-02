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

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::Arc;

use asyncband::mutex::Mutex;
use asyncband::once::OnceCell;
use bytes::Buf;
use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use http::request;
use serde::Deserialize;
use serde_json::json;

use opendal_core::raw::*;
use opendal_core::*;

#[derive(Clone)]
pub struct KoofrCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    /// The root of this core.
    pub root: String,
    /// The endpoint of this backend.
    pub endpoint: String,
    /// Koofr email
    pub email: String,
    /// Koofr password
    pub password: String,

    /// signer of this backend.
    pub signer: Arc<Mutex<KoofrSigner>>,

    // Koofr mount_id.
    pub mount_id: OnceCell<String>,
}

impl Debug for KoofrCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KoofrCore")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("email", &self.email)
            .finish_non_exhaustive()
    }
}

impl KoofrCore {
    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    pub async fn get_mount_id(&self, ctx: &OperationContext) -> Result<&String> {
        self.mount_id
            .get_or_try_init(|| async {
                let req = Request::get(format!("{}/api/v2/mounts", self.endpoint));

                let req = self.sign(ctx, req).await?;

                let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

                let resp = self.send(ctx, req).await?;

                let status = resp.status();

                if status != StatusCode::OK {
                    return Err(parse_error(
                        ErrorContext::new(ServiceOperation("ListMounts")),
                        resp,
                    ));
                }

                let bs = resp.into_body();

                let resp: MountsResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                for mount in resp.mounts {
                    if mount.is_primary {
                        return Ok(mount.id);
                    }
                }

                Err(Error::new(ErrorKind::Unexpected, "No primary mount found"))
            })
            .await
    }

    pub async fn sign(
        &self,
        ctx: &OperationContext,
        req: request::Builder,
    ) -> Result<request::Builder> {
        let mut signer = self.signer.lock().await;
        if !signer.token.is_empty() {
            return Ok(req.header(
                header::AUTHORIZATION,
                format!("Token token={}", signer.token),
            ));
        }

        let url = format!("{}/token", self.endpoint);

        let body = json!({
            "email": self.email,
            "password": self.password,
        });

        let bs = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;

        let auth_req = Request::post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Buffer::from(Bytes::from(bs)))
            .map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(auth_req).await?;

        let status = resp.status();

        if status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("Authenticate")),
                resp,
            ));
        }

        let bs = resp.into_body();
        let resp: TokenResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        signer.token = resp.token;

        Ok(req.header(
            header::AUTHORIZATION,
            format!("Token token={}", signer.token),
        ))
    }
}

impl KoofrCore {
    pub async fn ensure_dir_exists(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let mut dirs = VecDeque::default();

        let mut p = build_abs_path(&self.root, path);

        while p != "/" {
            let parent = get_parent(&p).to_string();

            dirs.push_front(parent.clone());
            p = parent;
        }

        for dir in dirs {
            self.create_dir(ctx, &dir).await?;
        }

        Ok(())
    }

    pub async fn create_dir(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let resp = self.info(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::NOT_FOUND => {
                let name = get_basename(path).trim_end_matches('/');
                let parent = get_parent(path);

                let mount_id = self.get_mount_id(ctx).await?;

                let url = format!(
                    "{}/api/v2/mounts/{}/files/folder?path={}",
                    self.endpoint,
                    mount_id,
                    percent_encode_path(parent)
                );

                let body = json!({
                    "name": name
                });

                let bs = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;

                let req = Request::post(url);

                let req = self.sign(ctx, req).await?;

                let req = req
                    .header(header::CONTENT_TYPE, "application/json")
                    .extension(Operation::CreateDir)
                    .extension(ServiceOperation("CreateFolder"))
                    .body(Buffer::from(Bytes::from(bs)))
                    .map_err(new_request_build_error)?;

                let resp = ctx.http_transport().send(req).await?;

                let status = resp.status();

                match status {
                    // When the directory already exists, Koofr returns 400 Bad Request.
                    // We should treat it as success.
                    StatusCode::OK | StatusCode::CREATED | StatusCode::BAD_REQUEST => Ok(()),
                    _ => Err(parse_error(
                        ErrorContext::new(ServiceOperation("CreateFolder")),
                        resp,
                    )),
                }
            }
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("FilesInfo")),
                resp,
            )),
        }
    }

    pub async fn info(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let mount_id = self.get_mount_id(ctx).await?;

        let url = format!(
            "{}/api/v2/mounts/{}/files/info?path={}",
            self.endpoint,
            mount_id,
            percent_encode_path(path)
        );

        let req = Request::get(url);

        let req = self.sign(ctx, req).await?;

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("FilesInfo"))
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
        let path = build_rooted_abs_path(&self.root, path);

        let mount_id = self.get_mount_id(ctx).await?;

        let url = format!(
            "{}/api/v2/mounts/{}/files/get?path={}",
            self.endpoint,
            mount_id,
            percent_encode_path(&path)
        );

        let req = Request::get(url).header(header::RANGE, range.to_header());

        let req = self.sign(ctx, req).await?;

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("FilesGet"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn put(
        &self,
        ctx: &OperationContext,
        path: &str,
        bs: Buffer,
    ) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);

        let filename = get_basename(&path);
        let parent = get_parent(&path);

        let mount_id = self.get_mount_id(ctx).await?;

        let url = format!(
            "{}/content/api/v2/mounts/{}/files/put?path={}&filename={}&info=true&overwriteIgnoreNonexisting=&autorename=false&overwrite=true",
            self.endpoint,
            mount_id,
            percent_encode_path(parent),
            percent_encode_path(filename)
        );

        let file_part = FormDataPart::new("file")
            .header(
                header::CONTENT_DISPOSITION,
                format!("form-data; name=\"file\"; filename=\"{filename}\"")
                    .parse()
                    .unwrap(),
            )
            .content(bs);

        let multipart = Multipart::new().part(file_part);

        let req = Request::post(url);

        let req = self.sign(ctx, req).await?;

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("FilesPut"));

        let req = multipart.apply(req)?;

        self.send(ctx, req).await
    }

    pub async fn remove(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);

        let mount_id = self.get_mount_id(ctx).await?;

        let url = format!(
            "{}/api/v2/mounts/{}/files/remove?path={}",
            self.endpoint,
            mount_id,
            percent_encode_path(&path)
        );

        let req = Request::delete(url);

        let req = self.sign(ctx, req).await?;

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("FilesRemove"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_rooted_abs_path(&self.root, from);
        let to = build_rooted_abs_path(&self.root, to);

        let mount_id = self.get_mount_id(ctx).await?;

        let url = format!(
            "{}/api/v2/mounts/{}/files/copy?path={}",
            self.endpoint,
            mount_id,
            percent_encode_path(&from),
        );

        let body = json!({
            "toMountId": mount_id,
            "toPath": to,
        });

        let bs = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;

        let req = Request::put(url);

        let req = self.sign(ctx, req).await?;

        let req = req
            .header(header::CONTENT_TYPE, "application/json")
            .extension(Operation::Copy)
            .extension(ServiceOperation("FilesCopy"))
            .body(Buffer::from(Bytes::from(bs)))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn move_object(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_rooted_abs_path(&self.root, from);
        let to = build_rooted_abs_path(&self.root, to);

        let mount_id = self.get_mount_id(ctx).await?;

        let url = format!(
            "{}/api/v2/mounts/{}/files/move?path={}",
            self.endpoint,
            mount_id,
            percent_encode_path(&from),
        );

        let body = json!({
            "toMountId": mount_id,
            "toPath": to,
        });

        let bs = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;

        let req = Request::put(url);

        let req = self.sign(ctx, req).await?;

        let req = req
            .header(header::CONTENT_TYPE, "application/json")
            .extension(Operation::Rename)
            .extension(ServiceOperation("FilesMove"))
            .body(Buffer::from(Bytes::from(bs)))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn list(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);

        let mount_id = self.get_mount_id(ctx).await?;

        let url = format!(
            "{}/api/v2/mounts/{}/files/list?path={}",
            self.endpoint,
            mount_id,
            percent_encode_path(&path)
        );

        let req = Request::get(url);

        let req = self.sign(ctx, req).await?;

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("FilesList"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }
}

#[derive(Clone, Default)]
pub struct KoofrSigner {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct MountsResponse {
    pub mounts: Vec<Mount>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    pub id: String,
    pub is_primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse {
    pub files: Vec<File>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub size: u64,
    pub modified: i64,
    pub content_type: String,
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
        304 | 412 => (ErrorKind::ConditionNotMatch, false),
        // Service like Koofr could return 499 error with a message like:
        // Client Disconnect, we should retry it.
        499 => (ErrorKind::Unexpected, true),
        500 | 502 | 503 | 504 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = String::from_utf8_lossy(&bs).into_owned();

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
        let err_res = vec![(r#""#, ErrorKind::NotFound, StatusCode::NOT_FOUND)];

        for res in err_res {
            let bs = bytes::Bytes::from(res.0);
            let body = Buffer::from(bs);
            let resp = Response::builder().status(res.2).body(body).unwrap();

            let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);

            assert_eq!(err.kind(), res.1);
        }
    }
}
