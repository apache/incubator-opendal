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
use http::Method;
use http::Request;
use http::Response;
use http::header;
use http::header::HeaderValue;
use serde::Deserialize;
use serde::Serialize;

use opendal_core::raw::*;
use opendal_core::*;

/// Available Aliyun Drive Type.
#[derive(Debug, Deserialize, Default, Clone)]
pub enum DriveType {
    /// Use the default type of Aliyun Drive.
    #[default]
    Default,
    /// Use the backup type of Aliyun Drive.
    ///
    /// Fallback to the default type if no backup drive is found.
    Backup,
    /// Use the resource type of Aliyun Drive.
    ///
    /// Fallback to the default type if no resource drive is found.
    Resource,
}

/// Available Aliyun Drive Signer Set
pub enum AliyunDriveSign {
    Refresh(String, String, String, Option<String>, i64),
    Access(String),
}

pub struct AliyunDriveSigner {
    pub drive_id: Option<String>,
    pub sign: AliyunDriveSign,
}

pub struct AliyunDriveCore {
    pub info: ServiceInfo,
    pub capability: Capability,

    pub endpoint: String,
    pub root: String,
    pub drive_type: DriveType,

    pub signer: Arc<Mutex<AliyunDriveSigner>>,
    pub dir_lock: Arc<Mutex<()>>,
}

impl Debug for AliyunDriveCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AliyunDriveCore")
            .field("root", &self.root)
            .field("drive_type", &self.drive_type)
            .finish_non_exhaustive()
    }
}

impl AliyunDriveCore {
    async fn send(
        &self,
        ctx: &OperationContext,
        mut req: Request<Buffer>,
        token: Option<&str>,
    ) -> Result<Response<Buffer>> {
        // AliyunDrive raise NullPointerException if you haven't set a user-agent.
        req.headers_mut().insert(
            header::USER_AGENT,
            HeaderValue::from_str(&format!("opendal/{VERSION}"))
                .expect("user agent must be valid header value"),
        );
        if req.method() == Method::POST {
            req.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json;charset=UTF-8"),
            );
        }
        if let Some(token) = token {
            req.headers_mut().insert(
                header::AUTHORIZATION,
                HeaderValue::from_str(&format_authorization_by_bearer(token)?)
                    .expect("access token must be valid header value"),
            );
        }
        ctx.http_transport().send(req).await
    }

    async fn get_access_token(
        &self,
        ctx: &OperationContext,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
    ) -> Result<Buffer> {
        let body = serde_json::to_vec(&AccessTokenRequest {
            refresh_token,
            grant_type: "refresh_token",
            client_id,
            client_secret,
        })
        .map_err(new_json_serialize_error)?;
        let req = Request::post(format!("{}/oauth/access_token", self.endpoint))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, None).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetAccessToken")),
                res,
            ));
        }
        Ok(res.into_body())
    }

    async fn get_drive_id(&self, ctx: &OperationContext, token: Option<&str>) -> Result<Buffer> {
        let req = Request::post(format!("{}/adrive/v1.0/user/getDriveInfo", self.endpoint))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetDriveInfo")),
                res,
            ));
        }
        Ok(res.into_body())
    }

    pub async fn get_token_and_drive(
        &self,
        ctx: &OperationContext,
    ) -> Result<(Option<String>, String)> {
        let mut signer = self.signer.lock().await;
        let token = match &mut signer.sign {
            AliyunDriveSign::Access(access_token) => Some(access_token.clone()),
            AliyunDriveSign::Refresh(
                client_id,
                client_secret,
                refresh_token,
                access_token,
                expire_at,
            ) => {
                if *expire_at < Timestamp::now().into_inner().as_second() || access_token.is_none()
                {
                    let res = self
                        .get_access_token(ctx, client_id, client_secret, refresh_token)
                        .await?;
                    let output: RefreshTokenResponse = serde_json::from_reader(res.reader())
                        .map_err(new_json_deserialize_error)?;
                    *access_token = Some(output.access_token);
                    *expire_at = output.expires_in + Timestamp::now().into_inner().as_second();
                    *refresh_token = output.refresh_token;
                }
                access_token.clone()
            }
        };
        let Some(drive_id) = &signer.drive_id else {
            let res = self.get_drive_id(ctx, token.as_deref()).await?;
            let output: DriveInfoResponse =
                serde_json::from_reader(res.reader()).map_err(new_json_deserialize_error)?;
            let drive_id = match self.drive_type {
                DriveType::Default => output.default_drive_id,
                DriveType::Backup => output.backup_drive_id.unwrap_or(output.default_drive_id),
                DriveType::Resource => output.resource_drive_id.unwrap_or(output.default_drive_id),
            };
            signer.drive_id = Some(drive_id.clone());
            return Ok((token, drive_id));
        };
        Ok((token, drive_id.clone()))
    }

    pub fn build_path(&self, path: &str, rooted: bool) -> String {
        let file_path = if rooted {
            build_rooted_abs_path(&self.root, path)
        } else {
            build_abs_path(&self.root, path)
        };
        let file_path = file_path.strip_suffix('/').unwrap_or(&file_path);
        if file_path.is_empty() {
            return "/".to_string();
        }
        file_path.to_string()
    }

    pub async fn get_by_path(&self, ctx: &OperationContext, path: &str) -> Result<Buffer> {
        let file_path = self.build_path(path, true);
        let req = Request::post(format!(
            "{}/adrive/v1.0/openFile/get_by_path",
            self.endpoint
        ));
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&GetByPathRequest {
            drive_id: &drive_id,
            file_path: &file_path,
        })
        .map_err(new_json_serialize_error)?;
        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetFileByPath"))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetFileByPath")),
                res,
            ));
        }
        Ok(res.into_body())
    }

    pub async fn ensure_dir_exists(&self, ctx: &OperationContext, path: &str) -> Result<String> {
        let file_path = self.build_path(path, false);
        if file_path == "/" {
            return Ok("root".to_string());
        }
        let file_path = file_path.strip_suffix('/').unwrap_or(&file_path);
        let paths = file_path.split('/').collect::<Vec<&str>>();
        let mut parent: Option<String> = None;
        for path in paths {
            let _guard = self.dir_lock.lock().await;
            let res = self
                .create(
                    ctx,
                    parent.as_deref(),
                    path,
                    CreateType::Folder,
                    CheckNameMode::Refuse,
                )
                .await?;
            let output: CreateResponse =
                serde_json::from_reader(res.reader()).map_err(new_json_deserialize_error)?;
            parent = Some(output.file_id);
        }
        Ok(parent.expect("ensure_dir_exists must succeed"))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_with_rapid_upload(
        &self,
        ctx: &OperationContext,
        parent_file_id: Option<&str>,
        name: &str,
        typ: CreateType,
        check_name_mode: CheckNameMode,
        size: Option<u64>,
        rapid_upload: Option<RapidUpload>,
    ) -> Result<Buffer> {
        let mut content_hash = None;
        let mut proof_code = None;
        let mut pre_hash = None;
        if let Some(rapid_upload) = rapid_upload {
            content_hash = rapid_upload.content_hash;
            proof_code = rapid_upload.proof_code;
            pre_hash = rapid_upload.pre_hash;
        }

        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&CreateRequest {
            drive_id: &drive_id,
            parent_file_id: parent_file_id.unwrap_or("root"),
            name,
            typ,
            check_name_mode,
            size,
            pre_hash: pre_hash.as_deref(),
            content_hash: content_hash.as_deref(),
            content_hash_name: content_hash.is_some().then_some("sha1"),
            proof_code: proof_code.as_deref(),
            proof_version: proof_code.is_some().then_some("v1"),
        })
        .map_err(new_json_serialize_error)?;
        let req = Request::post(format!("{}/adrive/v1.0/openFile/create", self.endpoint))
            .extension(Operation::Write)
            .extension(ServiceOperation("CreateFile"))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateFile")),
                res,
            ));
        }
        Ok(res.into_body())
    }

    pub async fn create(
        &self,
        ctx: &OperationContext,
        parent_file_id: Option<&str>,
        name: &str,
        typ: CreateType,
        check_name_mode: CheckNameMode,
    ) -> Result<Buffer> {
        self.create_with_rapid_upload(ctx, parent_file_id, name, typ, check_name_mode, None, None)
            .await
    }

    async fn get_download_url(&self, ctx: &OperationContext, file_id: &str) -> Result<String> {
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&FileRequest {
            drive_id: &drive_id,
            file_id,
        })
        .map_err(new_json_serialize_error)?;

        let req = Request::post(format!(
            "{}/adrive/v1.0/openFile/getDownloadUrl",
            self.endpoint
        ))
        .extension(Operation::Read)
        .extension(ServiceOperation("GetDownloadUrl"))
        .body(Buffer::from(body))
        .map_err(new_request_build_error)?;

        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetDownloadUrl")),
                res,
            ));
        }
        let res = res.into_body();

        let output: GetDownloadUrlResponse =
            serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;

        Ok(output.url)
    }

    pub async fn download(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let download_url = self.get_download_url(ctx, file_id).await?;
        let req = Request::get(download_url)
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadFile"))
            .header(header::RANGE, range.to_header())
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        ctx.http_transport().fetch(req).await
    }

    pub async fn move_path(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        to_parent_file_id: &str,
    ) -> Result<()> {
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&MovePathRequest {
            drive_id: &drive_id,
            file_id,
            to_parent_file_id,
            check_name_mode: CheckNameMode::AutoRename,
        })
        .map_err(new_json_serialize_error)?;
        let req = Request::post(format!("{}/adrive/v1.0/openFile/move", self.endpoint))
            .extension(Operation::Write)
            .extension(ServiceOperation("MoveFile"))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("MoveFile")),
                res,
            ));
        }
        Ok(())
    }

    pub async fn update_path(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        name: &str,
    ) -> Result<()> {
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&UpdatePathRequest {
            drive_id: &drive_id,
            file_id,
            name,
            check_name_mode: CheckNameMode::Refuse,
        })
        .map_err(new_json_serialize_error)?;
        let req = Request::post(format!("{}/adrive/v1.0/openFile/update", self.endpoint))
            .extension(Operation::Write)
            .extension(ServiceOperation("UpdateFile"))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("UpdateFile")),
                res,
            ));
        }
        Ok(())
    }

    pub async fn copy_path(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        to_parent_file_id: &str,
        auto_rename: bool,
    ) -> Result<Buffer> {
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&CopyPathRequest {
            drive_id: &drive_id,
            file_id,
            to_parent_file_id,
            auto_rename,
        })
        .map_err(new_json_serialize_error)?;
        let req = Request::post(format!("{}/adrive/v1.0/openFile/copy", self.endpoint))
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyFile"))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CopyFile")),
                res,
            ));
        }
        Ok(res.into_body())
    }

    pub async fn delete_path(&self, ctx: &OperationContext, file_id: &str) -> Result<()> {
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&FileRequest {
            drive_id: &drive_id,
            file_id,
        })
        .map_err(new_json_serialize_error)?;
        let req = Request::post(format!("{}/adrive/v1.0/openFile/delete", self.endpoint))
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteFile"))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteFile")),
                res,
            ));
        }
        Ok(())
    }

    pub async fn list(
        &self,
        ctx: &OperationContext,
        parent_file_id: &str,
        limit: Option<usize>,
        marker: Option<String>,
    ) -> Result<Buffer> {
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&ListRequest {
            drive_id: &drive_id,
            parent_file_id,
            limit,
            marker: marker.as_deref(),
        })
        .map_err(new_json_serialize_error)?;
        let req = Request::post(format!("{}/adrive/v1.0/openFile/list", self.endpoint))
            .extension(Operation::List)
            .extension(ServiceOperation("ListFiles"))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListFiles")),
                res,
            ));
        }
        Ok(res.into_body())
    }

    pub async fn complete(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        upload_id: &str,
    ) -> Result<Buffer> {
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let body = serde_json::to_vec(&CompleteRequest {
            drive_id: &drive_id,
            file_id,
            upload_id,
        })
        .map_err(new_json_serialize_error)?;
        let req = Request::post(format!("{}/adrive/v1.0/openFile/complete", self.endpoint))
            .extension(Operation::Write)
            .extension(ServiceOperation("CompleteUpload"))
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CompleteUpload")),
                res,
            ));
        }
        Ok(res.into_body())
    }

    async fn get_upload_url(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        upload_id: &str,
        part_number: usize,
    ) -> Result<String> {
        let (token, drive_id) = self.get_token_and_drive(ctx).await?;
        let part_info_list = vec![PartInfoItem {
            part_number: Some(part_number),
        }];
        let body = serde_json::to_vec(&GetUploadRequest {
            drive_id: &drive_id,
            file_id,
            upload_id,
            part_info_list: Some(part_info_list),
        })
        .map_err(new_json_serialize_error)?;

        let req = Request::post(format!(
            "{}/adrive/v1.0/openFile/getUploadUrl",
            self.endpoint
        ))
        .extension(Operation::Write)
        .extension(ServiceOperation("GetUploadUrl"))
        .body(Buffer::from(body))
        .map_err(new_request_build_error)?;

        let res = self.send(ctx, req, token.as_deref()).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetUploadUrl")),
                res,
            ));
        }
        let res = res.into_body();

        let mut output: UploadUrlResponse =
            serde_json::from_reader(res.reader()).map_err(new_json_deserialize_error)?;

        let Some(upload_url) = output
            .part_info_list
            .take()
            .map(|mut list| list.swap_remove(0))
            .map(|part_info| part_info.upload_url)
        else {
            return Err(Error::new(ErrorKind::Unexpected, "cannot find upload_url"));
        };

        Ok(upload_url)
    }
    pub async fn upload(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        upload_id: &str,
        part_number: usize,
        body: Buffer,
    ) -> Result<Buffer> {
        let upload_url = self
            .get_upload_url(ctx, file_id, upload_id, part_number)
            .await?;
        let req = Request::put(upload_url)
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadPart"))
            .body(body)
            .map_err(new_request_build_error)?;
        let res = self.send(ctx, req, None).await?;
        if !res.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadPart")),
                res,
            ));
        }
        Ok(res.into_body())
    }
}

pub struct RapidUpload {
    pub pre_hash: Option<String>,
    pub content_hash: Option<String>,
    pub proof_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct DriveInfoResponse {
    pub default_drive_id: String,
    pub resource_drive_id: Option<String>,
    pub backup_drive_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CreateType {
    File,
    Folder,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckNameMode {
    Refuse,
    AutoRename,
}

#[derive(Deserialize)]
pub struct UploadUrlResponse {
    pub part_info_list: Option<Vec<PartInfo>>,
}

#[derive(Deserialize)]
pub struct CreateResponse {
    pub file_id: String,
    pub upload_id: Option<String>,
    pub exist: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct PartInfo {
    pub etag: Option<String>,
    pub part_number: usize,
    pub part_size: Option<u64>,
    pub upload_url: String,
    pub content_type: Option<String>,
}

#[derive(Deserialize)]
pub struct AliyunDriveFileList {
    pub items: Vec<AliyunDriveFile>,
    pub next_marker: Option<String>,
}

#[derive(Deserialize)]
pub struct CopyResponse {
    pub file_id: String,
}

#[derive(Deserialize)]
pub struct AliyunDriveFile {
    pub file_id: String,
    pub parent_file_id: String,
    pub name: String,
    pub size: Option<u64>,
    pub content_type: Option<String>,
    #[serde(rename = "type")]
    pub path_type: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct GetDownloadUrlResponse {
    pub url: String,
}

#[derive(Serialize)]
pub struct AccessTokenRequest<'a> {
    refresh_token: &'a str,
    grant_type: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
}

#[derive(Serialize)]
pub struct GetByPathRequest<'a> {
    drive_id: &'a str,
    file_path: &'a str,
}

#[derive(Serialize)]
pub struct CreateRequest<'a> {
    drive_id: &'a str,
    parent_file_id: &'a str,
    name: &'a str,
    #[serde(rename = "type")]
    typ: CreateType,
    check_name_mode: CheckNameMode,
    size: Option<u64>,
    pre_hash: Option<&'a str>,
    content_hash: Option<&'a str>,
    content_hash_name: Option<&'a str>,
    proof_code: Option<&'a str>,
    proof_version: Option<&'a str>,
}

#[derive(Serialize)]
pub struct FileRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
}

#[derive(Serialize)]
pub struct MovePathRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    to_parent_file_id: &'a str,
    check_name_mode: CheckNameMode,
}

#[derive(Serialize)]
pub struct UpdatePathRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    name: &'a str,
    check_name_mode: CheckNameMode,
}

#[derive(Serialize)]
pub struct CopyPathRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    to_parent_file_id: &'a str,
    auto_rename: bool,
}

#[derive(Serialize)]
pub struct ListRequest<'a> {
    drive_id: &'a str,
    parent_file_id: &'a str,
    limit: Option<usize>,
    marker: Option<&'a str>,
}

#[derive(Serialize)]
pub struct CompleteRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    upload_id: &'a str,
}

#[derive(Serialize)]
pub struct GetUploadRequest<'a> {
    drive_id: &'a str,
    file_id: &'a str,
    upload_id: &'a str,
    part_info_list: Option<Vec<PartInfoItem>>,
}

#[derive(Serialize)]
pub struct PartInfoItem {
    part_number: Option<usize>,
}

#[derive(Default, Debug, Deserialize)]
struct AliyunDriveError {
    code: String,
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
pub(crate) fn parse_error(ctx: ErrorContext, res: Response<Buffer>) -> Error {
    let (parts, body) = res.into_parts();
    let bs = body.to_bytes();
    let (code, message) = serde_json::from_reader::<_, AliyunDriveError>(bs.clone().reader())
        .map(|err| (Some(err.code), err.message))
        .unwrap_or((None, String::from_utf8_lossy(&bs).into_owned()));
    let (kind, retryable) = match parts.status.as_u16() {
        403 => (ErrorKind::PermissionDenied, false),
        400 => match code {
            Some(code) if code == "NotFound.File" => (ErrorKind::NotFound, false),
            Some(code) if code == "AlreadyExist.File" => (ErrorKind::AlreadyExists, false),
            Some(code) if code == "PreHashMatched" => (ErrorKind::IsSameFile, false),
            _ => (ErrorKind::Unexpected, false),
        },
        409 => (ErrorKind::AlreadyExists, false),
        429 => match code {
            Some(code) if code == "TooManyRequests" => (ErrorKind::RateLimited, true),
            _ => (ErrorKind::Unexpected, false),
        },
        _ => (ErrorKind::Unexpected, false),
    };
    let mut err =
        Error::new(kind, message).with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);
    if retryable {
        err = err.set_temporary();
    }
    err
}
