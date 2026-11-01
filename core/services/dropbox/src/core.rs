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

use asyncband::mutex::Mutex;
use bytes::Buf;
use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use serde::Deserialize;
use serde::Serialize;

use opendal_core::raw::*;
use opendal_core::*;

pub struct DropboxCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub root: String,
    pub signer: Arc<Mutex<DropboxSigner>>,
}

impl Debug for DropboxCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropboxCore")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl DropboxCore {
    fn build_path(&self, path: &str) -> String {
        let path = build_rooted_abs_path(&self.root, path);
        // For dropbox, even the path is a directory,
        // we still need to remove the trailing slash.
        path.trim_end_matches('/').to_string()
    }

    pub async fn sign<T>(&self, ctx: &OperationContext, req: &mut Request<T>) -> Result<()> {
        let mut signer = self.signer.lock().await;

        // Access token is valid, use it directly.
        if !signer.access_token.is_empty() && signer.expires_in > Timestamp::now() {
            let value = format!("Bearer {}", signer.access_token)
                .parse()
                .expect("token must be valid header value");
            req.headers_mut().insert(header::AUTHORIZATION, value);
            return Ok(());
        }

        // Refresh invalid token.
        let url = "https://api.dropboxapi.com/oauth2/token".to_string();

        let content = format!(
            "grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
            signer.refresh_token, signer.client_id, signer.client_secret
        );
        let bs = Bytes::from(content);

        let request = Request::post(&url)
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(CONTENT_LENGTH, bs.len())
            .body(Buffer::from(bs))
            .map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(request).await?;
        let body = resp.into_body();

        let token: DropboxTokenResponse =
            serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

        // Update signer after token refreshed.
        signer.access_token.clone_from(&token.access_token);

        // Refresh it 2 minutes earlier.
        signer.expires_in =
            Timestamp::now() + Duration::from_secs(token.expires_in) - Duration::from_secs(120);

        let value = format!("Bearer {}", token.access_token)
            .parse()
            .expect("token must be valid header value");
        req.headers_mut().insert(header::AUTHORIZATION, value);

        Ok(())
    }

    pub async fn dropbox_get(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        _: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let url: String = "https://content.dropboxapi.com/2/files/download".to_string();
        let download_args = DropboxDownloadArgs {
            path: build_rooted_abs_path(&self.root, path),
        };
        let request_payload =
            serde_json::to_string(&download_args).map_err(new_json_serialize_error)?;

        let mut req = Request::post(&url)
            .header("Dropbox-API-Arg", request_payload)
            .header(CONTENT_LENGTH, 0);

        if !range.is_full() {
            req = req.header(header::RANGE, range.to_header());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadFile"));

        let mut request = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;
        ctx.http_transport().fetch(request).await
    }

    pub async fn dropbox_update(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: Option<usize>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let url = "https://content.dropboxapi.com/2/files/upload".to_string();
        let dropbox_update_args = DropboxUploadArgs {
            path: build_rooted_abs_path(&self.root, path),
            ..Default::default()
        };
        let mut request_builder = Request::post(&url);
        if let Some(size) = size {
            request_builder = request_builder.header(CONTENT_LENGTH, size);
        }
        request_builder = request_builder.header(
            CONTENT_TYPE,
            args.content_type().unwrap_or("application/octet-stream"),
        );

        let request_builder = request_builder
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadFile"));

        let mut request = request_builder
            .header(
                "Dropbox-API-Arg",
                serde_json::to_string(&dropbox_update_args).map_err(new_json_serialize_error)?,
            )
            .body(body)
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;
        ctx.http_transport().send(request).await
    }

    pub async fn dropbox_delete(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let url = "https://api.dropboxapi.com/2/files/delete_v2".to_string();
        let args = DropboxDeleteArgs {
            path: self.build_path(path),
        };

        let bs = Bytes::from(serde_json::to_string(&args).map_err(new_json_serialize_error)?);

        let mut request = Request::post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, bs.len())
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteFile"))
            .body(Buffer::from(bs))
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;
        ctx.http_transport().send(request).await
    }

    pub async fn dropbox_create_folder(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<RpCreateDir> {
        let url = "https://api.dropboxapi.com/2/files/create_folder_v2".to_string();
        let args = DropboxCreateFolderArgs {
            path: self.build_path(path),
        };

        let bs = Bytes::from(serde_json::to_string(&args).map_err(new_json_serialize_error)?);

        let mut request = Request::post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, bs.len())
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("CreateFolder"))
            .body(Buffer::from(bs))
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;
        let resp = ctx.http_transport().send(request).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => Ok(RpCreateDir::default()),
            _ => {
                let err = parse_error(ErrorContext::new(ServiceOperation("CreateFolder")), resp);
                match err.kind() {
                    ErrorKind::AlreadyExists => Ok(RpCreateDir::default()),
                    _ => Err(err),
                }
            }
        }
    }

    pub async fn dropbox_list(
        &self,
        ctx: &OperationContext,
        path: &str,
        recursive: bool,
        limit: Option<usize>,
    ) -> Result<Response<Buffer>> {
        let url = "https://api.dropboxapi.com/2/files/list_folder".to_string();

        // The default settings here align with the DropboxAPI default settings.
        // Refer: https://www.dropbox.com/developers/documentation/http/documentation#files-list_folder
        let args = DropboxListArgs {
            path: self.build_path(path),
            recursive,
            limit: limit.unwrap_or(1000),
        };

        let bs = Bytes::from(serde_json::to_string(&args).map_err(new_json_serialize_error)?);

        let mut request = Request::post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, bs.len())
            .extension(Operation::List)
            .extension(ServiceOperation("ListFolder"))
            .body(Buffer::from(bs))
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;
        ctx.http_transport().send(request).await
    }

    pub async fn dropbox_list_continue(
        &self,
        ctx: &OperationContext,
        cursor: &str,
    ) -> Result<Response<Buffer>> {
        let url = "https://api.dropboxapi.com/2/files/list_folder/continue".to_string();

        let args = DropboxListContinueArgs {
            cursor: cursor.to_string(),
        };

        let bs = Bytes::from(serde_json::to_string(&args).map_err(new_json_serialize_error)?);

        let mut request = Request::post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, bs.len())
            .extension(Operation::List)
            .extension(ServiceOperation("ListFolderContinue"))
            .body(Buffer::from(bs))
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;
        ctx.http_transport().send(request).await
    }

    pub async fn dropbox_copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let url = "https://api.dropboxapi.com/2/files/copy_v2".to_string();

        let args = DropboxCopyArgs {
            from_path: self.build_path(from),
            to_path: self.build_path(to),
        };

        let bs = Bytes::from(serde_json::to_string(&args).map_err(new_json_serialize_error)?);

        let mut request = Request::post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, bs.len())
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyFile"))
            .body(Buffer::from(bs))
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;
        ctx.http_transport().send(request).await
    }

    pub async fn dropbox_move(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let url = "https://api.dropboxapi.com/2/files/move_v2".to_string();

        let args = DropboxMoveArgs {
            from_path: self.build_path(from),
            to_path: self.build_path(to),
        };

        let bs = Bytes::from(serde_json::to_string(&args).map_err(new_json_serialize_error)?);

        let mut request = Request::post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, bs.len())
            .extension(Operation::Rename)
            .extension(ServiceOperation("MoveFile"))
            .body(Buffer::from(bs))
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;
        ctx.http_transport().send(request).await
    }

    pub async fn dropbox_get_metadata(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let url = "https://api.dropboxapi.com/2/files/get_metadata".to_string();
        let args = DropboxMetadataArgs {
            path: self.build_path(path),
            ..Default::default()
        };

        let bs = Bytes::from(serde_json::to_string(&args).map_err(new_json_serialize_error)?);

        let mut request = Request::post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header(CONTENT_LENGTH, bs.len())
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetMetadata"))
            .body(Buffer::from(bs))
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().send(request).await
    }
}

#[derive(Clone)]
pub struct DropboxSigner {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,

    pub access_token: String,
    pub expires_in: Timestamp,
}

impl Default for DropboxSigner {
    fn default() -> Self {
        DropboxSigner {
            refresh_token: String::new(),
            client_id: String::new(),
            client_secret: String::new(),

            access_token: String::new(),
            expires_in: Timestamp::MIN,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DropboxDownloadArgs {
    path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DropboxUploadArgs {
    path: String,
    mode: String,
    mute: bool,
    autorename: bool,
    strict_conflict: bool,
}

impl Default for DropboxUploadArgs {
    fn default() -> Self {
        DropboxUploadArgs {
            mode: "overwrite".to_string(),
            path: "".to_string(),
            mute: true,
            autorename: false,
            strict_conflict: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DropboxDeleteArgs {
    path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DropboxCreateFolderArgs {
    path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DropboxListArgs {
    path: String,
    recursive: bool,
    limit: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DropboxListContinueArgs {
    cursor: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DropboxCopyArgs {
    from_path: String,
    to_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DropboxMoveArgs {
    from_path: String,
    to_path: String,
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
struct DropboxMetadataArgs {
    include_deleted: bool,
    include_has_explicit_shared_members: bool,
    include_media_info: bool,
    path: String,
}

#[derive(Clone, Deserialize)]
struct DropboxTokenResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
pub struct DropboxMetadataResponse {
    #[serde(rename(deserialize = ".tag"))]
    pub tag: String,
    pub client_modified: String,
    pub content_hash: Option<String>,
    pub file_lock_info: Option<DropboxMetadataFileLockInfo>,
    pub has_explicit_shared_members: Option<bool>,
    pub id: String,
    pub is_downloadable: Option<bool>,
    pub name: String,
    pub path_display: String,
    pub path_lower: String,
    pub property_groups: Option<Vec<DropboxMetadataPropertyGroup>>,
    pub rev: Option<String>,
    pub server_modified: Option<String>,
    pub sharing_info: Option<DropboxMetadataSharingInfo>,
    pub size: Option<u64>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
pub struct DropboxMetadataFileLockInfo {
    pub created: Option<String>,
    pub is_lockholder: bool,
    pub lockholder_name: Option<String>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
pub struct DropboxMetadataPropertyGroup {
    pub fields: Vec<DropboxMetadataPropertyGroupField>,
    pub template_id: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
pub struct DropboxMetadataPropertyGroupField {
    pub name: String,
    pub value: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
pub struct DropboxMetadataSharingInfo {
    pub modified_by: Option<String>,
    pub parent_shared_folder_id: Option<String>,
    pub read_only: Option<bool>,
    pub shared_folder_id: Option<String>,
    pub traverse_only: Option<bool>,
    pub no_access: Option<bool>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
pub struct DropboxListResponse {
    pub entries: Vec<DropboxMetadataResponse>,
    pub cursor: String,
    pub has_more: bool,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
pub struct DropboxErrorResponse {
    pub error_summary: String,
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

    let (mut kind, mut retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::TOO_MANY_REQUESTS => (ErrorKind::RateLimited, true),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, dropbox_err) = serde_json::from_slice::<DropboxErrorResponse>(&bs)
        .map(|dropbox_err| (format!("{dropbox_err:?}"), Some(dropbox_err)))
        .unwrap_or_else(|_| (String::from_utf8_lossy(&bs).into_owned(), None));

    if let Some(dropbox_err) = dropbox_err {
        (kind, retryable) =
            parse_dropbox_error_summary(&dropbox_err.error_summary).unwrap_or((kind, retryable));
    }

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

/// We cannot get the error type from the response header when the status code is 409.
/// Because Dropbox API v2 will put error summary in the response body,
/// we need to parse it to get the correct error type and then error kind.
///
/// See <https://www.dropbox.com/developers/documentation/http/documentation#error-handling>
pub fn parse_dropbox_error_summary(summary: &str) -> Option<(ErrorKind, bool)> {
    if summary.starts_with("path/not_found")
        || summary.starts_with("path_lookup/not_found")
        || summary.starts_with("from_lookup/not_found")
    {
        Some((ErrorKind::NotFound, false))
    } else if summary.starts_with("path/conflict") {
        Some((ErrorKind::AlreadyExists, false))
    } else if summary.starts_with("too_many_write_operations") {
        Some((ErrorKind::RateLimited, true))
    } else {
        None
    }
}
