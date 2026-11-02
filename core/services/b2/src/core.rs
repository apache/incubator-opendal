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
use std::sync::Arc;

use asyncband::rwlock::RwLock;
use bytes::Buf;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use serde::Deserialize;
use serde::Serialize;

use self::constants::X_BZ_CONTENT_SHA1;
use self::constants::X_BZ_FILE_NAME;
use constants::X_BZ_PART_NUMBER;
use opendal_core::raw::*;
use opendal_core::*;

pub(super) mod constants {
    pub const X_BZ_FILE_NAME: &str = "X-Bz-File-Name";
    pub const X_BZ_CONTENT_SHA1: &str = "X-Bz-Content-Sha1";
    pub const X_BZ_PART_NUMBER: &str = "X-Bz-Part-Number";
    pub const X_BZ_INFO_PREFIX: &str = "X-Bz-Info-";
}

/// Core of [b2](https://www.backblaze.com/cloud-storage) services support.
#[derive(Clone)]
pub struct B2Core {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub signer: Arc<RwLock<B2Signer>>,

    /// The root of this core.
    pub root: String,
    /// The bucket name of this backend.
    pub bucket: String,
    /// The bucket id of this backend.
    pub bucket_id: String,
}

impl Debug for B2Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("B2Core")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("bucket_id", &self.bucket_id)
            .finish_non_exhaustive()
    }
}

impl B2Core {
    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    /// [b2_authorize_account](https://www.backblaze.com/apidocs/b2-authorize-account)
    pub async fn get_auth_info(&self, ctx: &OperationContext) -> Result<AuthInfo> {
        {
            let signer = self.signer.read().await;

            if !signer.auth_info.authorization_token.is_empty()
                && signer.auth_info.expires_in > Timestamp::now()
            {
                let auth_info = signer.auth_info.clone();
                return Ok(auth_info);
            }
        }

        {
            let mut signer = self.signer.write().await;
            let req = Request::get("https://api.backblazeb2.com/b2api/v2/b2_authorize_account")
                .header(
                    header::AUTHORIZATION,
                    format_authorization_by_basic(
                        &signer.application_key_id,
                        &signer.application_key,
                    )?,
                )
                .body(Buffer::new())
                .map_err(new_request_build_error)?;

            let resp = ctx.http_transport().send(req).await?;
            let status = resp.status();

            match status {
                StatusCode::OK => {
                    let resp_body = resp.into_body();
                    let token: AuthorizeAccountResponse =
                        serde_json::from_reader(resp_body.reader())
                            .map_err(new_json_deserialize_error)?;
                    signer.auth_info = AuthInfo {
                        authorization_token: token.authorization_token.clone(),
                        api_url: token.api_url.clone(),
                        download_url: token.download_url.clone(),
                        // This authorization token is valid for at most 24 hours.
                        expires_in: Timestamp::now() + Duration::from_secs(20 * 60 * 60),
                    };
                }
                _ => {
                    return Err(parse_error(
                        ErrorContext::new(ServiceOperation("AuthorizeAccount")),
                        resp,
                    ));
                }
            }
            Ok(signer.auth_info.clone())
        }
    }
}

impl B2Core {
    pub async fn download_file_by_name(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        _args: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let path = build_abs_path(&self.root, path);

        let auth_info = self.get_auth_info(ctx).await?;

        // Construct headers to add to the request
        let url = format!(
            "{}/file/{}/{}",
            auth_info.download_url,
            self.bucket,
            percent_encode_path(&path)
        );

        let mut req = Request::get(&url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        if !range.is_full() {
            req = req.header(header::RANGE, range.to_header());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadFileByName"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub(super) async fn get_upload_url(
        &self,
        ctx: &OperationContext,
    ) -> Result<GetUploadUrlResponse> {
        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!(
            "{}/b2api/v2/b2_get_upload_url?bucketId={}",
            auth_info.api_url, self.bucket_id
        );

        let mut req = Request::get(&url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("GetUploadUrl"));

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let resp_body = resp.into_body();
                let resp = serde_json::from_reader(resp_body.reader())
                    .map_err(new_json_deserialize_error)?;
                Ok(resp)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetUploadUrl")),
                resp,
            )),
        }
    }

    pub async fn get_download_authorization(
        &self,
        ctx: &OperationContext,
        path: &str,
        expire: Duration,
    ) -> Result<GetDownloadAuthorizationResponse> {
        let path = build_abs_path(&self.root, path);

        let auth_info = self.get_auth_info(ctx).await?;

        // Construct headers to add to the request
        let url = format!(
            "{}/b2api/v2/b2_get_download_authorization",
            auth_info.api_url
        );
        let mut req = Request::post(&url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        let body = GetDownloadAuthorizationRequest {
            bucket_id: self.bucket_id.clone(),
            file_name_prefix: path,
            valid_duration_in_seconds: expire.as_secs(),
        };
        let body = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;
        let body = bytes::Bytes::from(body);

        let req = req
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let resp_body = resp.into_body();
                let resp = serde_json::from_reader(resp_body.reader())
                    .map_err(new_json_deserialize_error)?;
                Ok(resp)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetDownloadAuthorization")),
                resp,
            )),
        }
    }

    pub async fn upload_file(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let resp = self.get_upload_url(ctx).await?;

        let p = build_abs_path(&self.root, path);

        let mut req = Request::post(resp.upload_url);

        req = req.header(X_BZ_FILE_NAME, percent_encode_path(&p));

        req = req.header(header::AUTHORIZATION, resp.authorization_token);

        req = req.header(X_BZ_CONTENT_SHA1, "do_not_verify");

        if let Some(size) = size {
            req = req.header(header::CONTENT_LENGTH, size.to_string())
        }

        if let Some(mime) = args.content_type() {
            req = req.header(header::CONTENT_TYPE, mime)
        } else {
            req = req.header(header::CONTENT_TYPE, "b2/x-auto")
        }

        if let Some(pos) = args.content_disposition() {
            req = req.header(header::CONTENT_DISPOSITION, pos)
        }

        // Set user metadata headers.
        // B2 uses X-Bz-Info-* prefix for custom file info.
        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(
                    format!("{}{}", constants::X_BZ_INFO_PREFIX, key),
                    percent_encode_path(value),
                );
            }
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadFile"));

        // Set body
        let req = req.body(body).map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn start_large_file(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!("{}/b2api/v2/b2_start_large_file", auth_info.api_url);

        let mut req = Request::post(&url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        let mut start_large_file_request = StartLargeFileRequest {
            bucket_id: self.bucket_id.clone(),
            file_name: percent_encode_path(&p),
            content_type: "b2/x-auto".to_owned(),
            file_info: None,
        };

        if let Some(mime) = args.content_type() {
            mime.clone_into(&mut start_large_file_request.content_type)
        }

        // Set user metadata in file_info.
        // B2 uses fileInfo field for custom file info in start_large_file API.
        if let Some(user_metadata) = args.user_metadata() {
            let file_info: HashMap<String, String> = user_metadata
                .into_iter()
                .map(|(k, v)| (k.to_string(), percent_encode_path(v)))
                .collect();
            start_large_file_request.file_info = Some(file_info);
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("StartLargeFile"));

        let body =
            serde_json::to_vec(&start_large_file_request).map_err(new_json_serialize_error)?;
        let body = bytes::Bytes::from(body);

        let req = req
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn get_upload_part_url(
        &self,
        ctx: &OperationContext,
        file_id: &str,
    ) -> Result<GetUploadPartUrlResponse> {
        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!(
            "{}/b2api/v2/b2_get_upload_part_url?fileId={}",
            auth_info.api_url, file_id
        );

        let mut req = Request::get(&url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("GetUploadPartUrl"));

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let resp_body = resp.into_body();
                let resp = serde_json::from_reader(resp_body.reader())
                    .map_err(new_json_deserialize_error)?;
                Ok(resp)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetUploadPartUrl")),
                resp,
            )),
        }
    }

    pub async fn upload_part(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let resp = self.get_upload_part_url(ctx, file_id).await?;

        let mut req = Request::post(resp.upload_url);

        req = req.header(X_BZ_PART_NUMBER, part_number.to_string());

        req = req.header(header::CONTENT_LENGTH, size.to_string());

        req = req.header(header::AUTHORIZATION, resp.authorization_token);

        req = req.header(X_BZ_CONTENT_SHA1, "do_not_verify");

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadPart"));

        // Set body
        let req = req.body(body).map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn finish_large_file(
        &self,
        ctx: &OperationContext,
        file_id: &str,
        part_sha1_array: Vec<String>,
    ) -> Result<Response<Buffer>> {
        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!("{}/b2api/v2/b2_finish_large_file", auth_info.api_url);

        let mut req = Request::post(&url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("FinishLargeFile"));

        let body = serde_json::to_vec(&FinishLargeFileRequest {
            file_id: file_id.to_owned(),
            part_sha1_array,
        })
        .map_err(new_json_serialize_error)?;
        let body = bytes::Bytes::from(body);

        // Set body
        let req = req
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn cancel_large_file(
        &self,
        ctx: &OperationContext,
        file_id: &str,
    ) -> Result<Response<Buffer>> {
        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!("{}/b2api/v2/b2_cancel_large_file", auth_info.api_url);

        let mut req = Request::post(&url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CancelLargeFile"));

        let body = serde_json::to_vec(&CancelLargeFileRequest {
            file_id: file_id.to_owned(),
        })
        .map_err(new_json_serialize_error)?;
        let body = bytes::Bytes::from(body);

        // Set body
        let req = req
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn get_file_info(
        &self,
        ctx: &OperationContext,
        path: &str,
        delimiter: Option<&str>,
    ) -> Result<File> {
        let resp = self
            .list_file_names_raw(ctx, Some(path), delimiter, None, None, Operation::Stat)
            .await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let bs = resp.into_body();
                let mut resp: ListFileNamesResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                if resp.files.is_empty() {
                    return Err(Error::new(ErrorKind::NotFound, "no such file or directory"));
                }
                Ok(resp.files.swap_remove(0))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("ListFileNames")),
                resp,
            )),
        }
    }

    pub async fn list_file_names(
        &self,
        ctx: &OperationContext,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        limit: Option<usize>,
        start_after: Option<String>,
    ) -> Result<Response<Buffer>> {
        self.list_file_names_raw(ctx, prefix, delimiter, limit, start_after, Operation::List)
            .await
    }

    async fn list_file_names_raw(
        &self,
        ctx: &OperationContext,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        limit: Option<usize>,
        start_after: Option<String>,
        operation: Operation,
    ) -> Result<Response<Buffer>> {
        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!("{}/b2api/v2/b2_list_file_names", auth_info.api_url);

        let mut url = QueryPairsWriter::new(&url);
        url = url.push("bucketId", &self.bucket_id);

        if let Some(prefix) = prefix {
            let prefix = build_abs_path(&self.root, prefix);
            if !prefix.is_empty() {
                url = url.push("prefix", &percent_encode_path(&prefix));
            }
        }

        if let Some(limit) = limit {
            url = url.push("maxFileCount", &limit.to_string());
        }

        if let Some(start_after) = start_after {
            url = url.push("startFileName", &percent_encode_path(&start_after));
        }

        if let Some(delimiter) = delimiter {
            url = url.push("delimiter", delimiter);
        }

        let mut req = Request::get(url.finish());

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        req = req
            .extension(operation)
            .extension(ServiceOperation("ListFileNames"));

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn copy_file(
        &self,
        ctx: &OperationContext,
        source_file_id: String,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let to = build_abs_path(&self.root, to);

        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!("{}/b2api/v2/b2_copy_file", auth_info.api_url);

        let mut req = Request::post(url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyFile"));

        let body = CopyFileRequest {
            source_file_id,
            file_name: to,
        };

        let body = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;
        let body = bytes::Bytes::from(body);

        // Set body
        let req = req
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn hide_file(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let auth_info = self.get_auth_info(ctx).await?;

        let url = format!("{}/b2api/v2/b2_hide_file", auth_info.api_url);

        let mut req = Request::post(url);

        req = req.header(header::AUTHORIZATION, auth_info.authorization_token);

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("HideFile"));

        let body = HideFileRequest {
            bucket_id: self.bucket_id.clone(),
            file_name: path.to_string(),
        };

        let body = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;
        let body = bytes::Bytes::from(body);

        // Set body
        let req = req
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }
}

#[derive(Clone)]
pub struct B2Signer {
    /// The application_key_id of this core.
    pub application_key_id: String,
    /// The application_key of this core.
    pub application_key: String,

    pub auth_info: AuthInfo,
}

#[derive(Clone)]
pub struct AuthInfo {
    pub authorization_token: String,
    /// The base URL to use for all API calls except for uploading and downloading files.
    pub api_url: String,
    /// The base URL to use for downloading files.
    pub download_url: String,
    /// The time when the authorization token expires.
    pub expires_in: Timestamp,
}

impl Default for B2Signer {
    fn default() -> Self {
        B2Signer {
            application_key: String::new(),
            application_key_id: String::new(),

            auth_info: AuthInfo {
                authorization_token: String::new(),
                api_url: String::new(),
                download_url: String::new(),
                expires_in: Timestamp::MIN,
            },
        }
    }
}

/// Request of [b2_start_large_file](https://www.backblaze.com/apidocs/b2-start-large-file).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartLargeFileRequest {
    pub bucket_id: String,
    pub file_name: String,
    pub content_type: String,
    /// Custom file info (user metadata) to store with the file.
    /// Keys should NOT include the `X-Bz-Info-` prefix.
    /// Values should be URL-encoded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_info: Option<HashMap<String, String>>,
}

/// Response of [b2_start_large_file](https://www.backblaze.com/apidocs/b2-start-large-file).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartLargeFileResponse {
    pub file_id: String,
}

/// Response of [b2_authorize_account](https://www.backblaze.com/apidocs/b2-authorize-account).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeAccountResponse {
    /// An authorization token to use with all calls, other than b2_authorize_account, that need an Authorization header. This authorization token is valid for at most 24 hours.
    /// So we should call b2_authorize_account every 24 hours.
    pub authorization_token: String,
    pub api_url: String,
    pub download_url: String,
}

/// Response of [b2_get_upload_url](https://www.backblaze.com/apidocs/b2-get-upload-url).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUploadUrlResponse {
    /// The authorizationToken that must be used when uploading files to this bucket.
    /// This token is valid for 24 hours or until the uploadUrl endpoint rejects an upload, see b2_upload_file
    pub authorization_token: String,
    pub upload_url: String,
}

/// Response of [b2_get_upload_url](https://www.backblaze.com/apidocs/b2-get-upload-part-url).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetUploadPartUrlResponse {
    /// The authorizationToken that must be used when uploading files to this bucket.
    /// This token is valid for 24 hours or until the uploadUrl endpoint rejects an upload, see b2_upload_file
    pub authorization_token: String,
    pub upload_url: String,
}

/// Response of [b2_upload_part](https://www.backblaze.com/apidocs/b2-upload-part).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPartResponse {
    pub content_sha1: String,
}

/// Response of [b2_finish_large_file](https://www.backblaze.com/apidocs/b2-finish-large-file).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishLargeFileRequest {
    pub file_id: String,
    pub part_sha1_array: Vec<String>,
}

/// Response of [b2_cancel_large_file](https://www.backblaze.com/apidocs/b2-cancel-large-file).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelLargeFileRequest {
    pub file_id: String,
}

/// Response of [list_file_names](https://www.backblaze.com/apidocs/b2-list-file-names).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFileNamesResponse {
    pub files: Vec<File>,
    pub next_file_name: Option<String>,
}

/// Response of [b2-finish-large-file](https://www.backblaze.com/apidocs/b2-finish-large-file).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    pub content_length: u64,
    pub content_md5: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub file_id: Option<String>,
    pub content_length: u64,
    pub content_md5: Option<String>,
    pub content_type: Option<String>,
    pub file_name: String,
    /// Custom file info (user metadata) stored with the file.
    /// Keys are the original names without the `X-Bz-Info-` prefix.
    /// Values are URL-encoded when stored, decoded when read.
    #[serde(default)]
    pub file_info: HashMap<String, String>,
}

pub(super) fn parse_file_info(file: &File) -> Metadata {
    if file.file_name.ends_with('/') {
        return MetadataBuilder::dir().build();
    }

    let mut metadata = MetadataBuilder::file(file.content_length);

    if let Some(content_md5) = &file.content_md5 {
        metadata.content_md5(content_md5);
    }

    if let Some(content_type) = &file.content_type {
        metadata.content_type(content_type);
    }

    // Parse user metadata from file_info
    // B2 stores user metadata with keys stripped of the "X-Bz-Info-" prefix
    // and values are URL-encoded
    if !file.file_info.is_empty() {
        let user_metadata: HashMap<String, String> = file
            .file_info
            .iter()
            .map(|(k, v)| (k.to_lowercase(), percent_decode_path(v)))
            .collect();
        if !user_metadata.is_empty() {
            metadata.user_metadata(user_metadata);
        }
    }

    metadata.build()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyFileRequest {
    pub source_file_id: String,
    pub file_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HideFileRequest {
    pub bucket_id: String,
    pub file_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDownloadAuthorizationRequest {
    pub bucket_id: String,
    pub file_name_prefix: String,
    pub valid_duration_in_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDownloadAuthorizationResponse {
    pub authorization_token: String,
}

/// the error response of b2
#[derive(Default, Debug, Deserialize)]
#[allow(dead_code)]
struct B2Error {
    status: u32,
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
pub(crate) fn parse_error(ctx: ErrorContext, resp: Response<Buffer>) -> Error {
    let (parts, body) = resp.into_parts();
    let bs = body.to_bytes();

    let (mut kind, mut retryable) = match parts.status.as_u16() {
        403 => (ErrorKind::PermissionDenied, false),
        404 => (ErrorKind::NotFound, false),
        304 | 412 => (ErrorKind::ConditionNotMatch, false),
        // Service b2 could return 403, show the authorization error
        401 => (ErrorKind::PermissionDenied, true),
        429 => (ErrorKind::RateLimited, true),
        500 | 502 | 503 | 504 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, b2_err) = serde_json::from_reader::<_, B2Error>(bs.clone().reader())
        .map(|b2_err| (format!("{b2_err:?}"), Some(b2_err)))
        .unwrap_or_else(|_| (String::from_utf8_lossy(&bs).into_owned(), None));

    if let Some(b2_err) = b2_err {
        (kind, retryable) = parse_b2_error_code(b2_err.code.as_str()).unwrap_or((kind, retryable));
    };

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

/// Returns the `Error kind` of this code and whether the error is retryable.
pub(crate) fn parse_b2_error_code(code: &str) -> Option<(ErrorKind, bool)> {
    match code {
        "already_hidden" => Some((ErrorKind::AlreadyExists, false)),
        "no_such_file" => Some((ErrorKind::NotFound, false)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use http::StatusCode;

    use super::*;

    #[test]
    fn test_parse_b2_error_code() {
        let code = "already_hidden";
        assert_eq!(
            parse_b2_error_code(code),
            Some((opendal_core::ErrorKind::AlreadyExists, false))
        );

        let code = "no_such_file";
        assert_eq!(
            parse_b2_error_code(code),
            Some((opendal_core::ErrorKind::NotFound, false))
        );

        let code = "not_found";
        assert_eq!(parse_b2_error_code(code), None);
    }

    #[tokio::test]
    async fn test_parse_error() {
        let err_res = vec![
            (
                r#"{"status": 403, "code": "access_denied", "message":"The provided customer-managed encryption key is wrong."}"#,
                ErrorKind::PermissionDenied,
                StatusCode::FORBIDDEN,
            ),
            (
                r#"{"status": 404, "code": "not_found", "message":"File is not in B2 Cloud Storage."}"#,
                ErrorKind::NotFound,
                StatusCode::NOT_FOUND,
            ),
            (
                r#"{"status": 401, "code": "bad_auth_token", "message":"The auth token used is not valid. Call b2_authorize_account again to either get a new one, or an error message describing the problem."}"#,
                ErrorKind::PermissionDenied,
                StatusCode::UNAUTHORIZED,
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
