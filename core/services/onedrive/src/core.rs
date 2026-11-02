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
use std::time::Duration;

use asyncband::mutex::Mutex;
use bytes::Buf;
use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;

use opendal_core::raw::*;
use opendal_core::*;

use super::graph_model::*;

pub struct OneDriveCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub root: String,
    pub signer: Arc<Mutex<OneDriveSigner>>,
}

impl Debug for OneDriveCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OneDriveCore")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

// OneDrive returns 400 when try to access a dir with the POSIX special directory entries
const SPECIAL_POSIX_ENTRIES: [&str; 3] = [".", "/", ""];

// organizes a few core module functions
impl OneDriveCore {
    // OneDrive personal's base URL. `me` is an alias that represents the user's "Drive".
    pub(crate) const DRIVE_ROOT_URL: &'static str =
        "https://graph.microsoft.com/v1.0/me/drive/root";

    /// Get a URL to an OneDrive item
    pub(crate) fn onedrive_item_url(&self, path: &str, build_absolute_path: bool) -> String {
        // OneDrive requires the root to be the same as `DRIVE_ROOT_URL`.
        // For files under the root, the URL pattern becomes `https://graph.microsoft.com/v1.0/me/drive/root:<path>:`
        if self.root == "/" && SPECIAL_POSIX_ENTRIES.contains(&path) {
            Self::DRIVE_ROOT_URL.to_string()
        } else {
            // OneDrive returns 400 when try to access a folder with a ending slash
            let absolute_path = if build_absolute_path {
                let rooted_path = build_rooted_abs_path(&self.root, path);
                rooted_path
                    .strip_suffix('/')
                    .unwrap_or(rooted_path.as_str())
                    .to_string()
            } else {
                path.to_string()
            };
            format!(
                "{}:{}",
                Self::DRIVE_ROOT_URL,
                percent_encode_path(&absolute_path),
            )
        }
    }

    /// Send a simplest stat request about a particular path
    ///
    /// See also: [`onedrive_stat()`].
    pub(crate) async fn onedrive_get_stat_plain(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let url: String = format!(
            "{}?{}",
            self.onedrive_item_url(path, true),
            GENERAL_SELECT_PARAM
        );
        let request = Request::get(&url);

        let mut request = request
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetItem"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().send(request).await
    }

    /// Create a directory at path if not exist, return the metadata about the folder
    ///
    /// When the folder exist, this function works exactly the same as [`onedrive_get_stat_plain()`].
    ///
    /// * `path` - a relative folder path
    pub(crate) async fn ensure_directory(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<OneDriveItem> {
        let response = self.onedrive_get_stat_plain(ctx, path).await?;
        let item: OneDriveItem = match response.status() {
            StatusCode::OK => {
                let bytes = response.into_body();
                serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?
            }
            StatusCode::NOT_FOUND => {
                // We must create directory for the destination
                let response = self.onedrive_create_dir(ctx, path).await?;
                match response.status() {
                    StatusCode::CREATED | StatusCode::OK => {
                        let bytes = response.into_body();
                        serde_json::from_reader(bytes.reader())
                            .map_err(new_json_deserialize_error)?
                    }
                    _ => {
                        return Err(parse_error(
                            ErrorContext::new(ServiceOperation("GetItem")),
                            response,
                        ));
                    }
                }
            }
            _ => {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("GetItem")),
                    response,
                ));
            }
        };

        Ok(item)
    }

    pub(crate) async fn sign<T>(
        &self,
        ctx: &OperationContext,
        request: &mut Request<T>,
    ) -> Result<()> {
        let mut signer = self.signer.lock().await;
        signer.sign(ctx, request).await
    }
}

// OneDrive copy action is asynchronous. We query an endpoint and wait 1 second.
// This is the maximum attempts we will wait.
const MAX_MONITOR_ATTEMPT: i32 = 3600;
const MONITOR_WAIT_SECOND: u64 = 1;

// OneDrive API parameters allows using with a parameter of:
//
// - ID
// - file path
//
// `services-onedrive` uses the file path based API for simplicity.
// Read more at https://learn.microsoft.com/en-us/graph/onedrive-addressing-driveitems
impl OneDriveCore {
    /// Send a stat request about a particular path.
    pub(crate) async fn onedrive_stat(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpStat,
    ) -> Result<Metadata> {
        let url: String = self.onedrive_item_url(path, true);

        let mut request = Request::get(&url);
        if let Some(etag) = args.if_none_match() {
            request = request.header(header::IF_NONE_MATCH, etag);
        }

        let mut request = request
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetItem"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        let response = ctx.http_transport().send(request).await?;
        if !response.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetItem"))
                    .with_caller_condition(args.is_conditional()),
                response,
            ));
        }

        let bytes = response.into_body();
        let decoded_response: OneDriveItem =
            serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;

        let entry_mode: EntryMode = match decoded_response.item_type {
            ItemType::Folder { .. } => EntryMode::DIR,
            ItemType::File { .. } => EntryMode::FILE,
        };

        let mut meta = if entry_mode == EntryMode::FILE {
            MetadataBuilder::file(decoded_response.size.max(0) as u64)
        } else {
            MetadataBuilder::dir()
        };
        meta.etag(decoded_response.e_tag);

        let last_modified = decoded_response.last_modified_date_time;
        let date_utc_last_modified = last_modified.parse::<Timestamp>()?;
        meta.last_modified(date_utc_last_modified);

        Ok(meta.build())
    }

    /// Return versions of an item
    ///
    /// A folder has no versions.
    ///
    /// * `path` - a relative path
    pub(crate) async fn onedrive_list_versions(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Vec<OneDriveItemVersion>> {
        // don't `$select` this endpoint to get the download URL.
        let url: String = format!(
            "{}:/versions?{}",
            self.onedrive_item_url(path, true),
            VERSION_SELECT_PARAM
        );

        let mut request = Request::get(url)
            .extension(Operation::List)
            .extension(ServiceOperation("ListVersions"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        let response = ctx.http_transport().send(request).await?;
        if !response.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListVersions")),
                response,
            ));
        }
        let decoded_response: GraphApiOneDriveVersionsResponse =
            serde_json::from_reader(response.into_body().reader())
                .map_err(new_json_deserialize_error)?;
        Ok(decoded_response.value)
    }

    pub(crate) fn onedrive_list_request(
        &self,
        path: &str,
        limit: Option<usize>,
    ) -> Result<Request<Buffer>> {
        let item_url = self.onedrive_item_url(path, true);
        // Root is addressed as `root/children`, not the path form `root:/children`.
        let mut url = if item_url == Self::DRIVE_ROOT_URL {
            format!("{item_url}/children?{GENERAL_SELECT_PARAM}")
        } else {
            format!("{item_url}:/children?{GENERAL_SELECT_PARAM}")
        };
        if let Some(limit) = limit {
            url += &format!("&$top={limit}");
        }

        Request::get(&url)
            .extension(Operation::List)
            .extension(ServiceOperation("ListChildren"))
            .body(Buffer::new())
            .map_err(new_request_build_error)
    }

    pub(crate) async fn onedrive_list(
        &self,
        ctx: &OperationContext,
        path: &str,
        limit: Option<usize>,
    ) -> Result<Response<Buffer>> {
        let mut request = self.onedrive_list_request(path, limit)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().send(request).await
    }

    pub(crate) async fn onedrive_get_next_list_page(
        &self,
        ctx: &OperationContext,
        url: &str,
    ) -> Result<Response<Buffer>> {
        let mut request = Request::get(url)
            .extension(Operation::List)
            .extension(ServiceOperation("ListChildren"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().send(request).await
    }

    /// Download a file
    ///
    /// OneDrive handles a download in 2 steps:
    /// 1. Returns a 302 with a presigned URL. If `If-None-Match` succeed, returns 304.
    /// 2. With the presigned URL, we can send a GET:
    ///   1. When getting an item succeed with a `Range` header, we get a 206 Partial Content response.
    ///   2. When succeed, we get a 200 response.
    ///
    /// Read more at https://learn.microsoft.com/en-us/graph/api/driveitem-get-content
    pub(crate) async fn onedrive_get_content(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Response<HttpBody>> {
        // We can't "select" the OneDrive API response fields when reading because "select" shadows not found error
        let url: String = format!("{}:/content", self.onedrive_item_url(path, true));

        let mut request = Request::get(&url).header(header::RANGE, range.to_header());
        if let Some(etag) = args.if_none_match() {
            request = request.header(header::IF_NONE_MATCH, etag);
        }

        let mut request = request
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadContent"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().fetch(request).await
    }

    /// Upload a file
    ///
    /// When creating a file,
    ///
    /// * OneDrive returns 201 if the file is new.
    /// * OneDrive returns 200 if successfully overwrote the file successfully.
    ///
    /// Read more at https://learn.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_put_content
    ///
    /// This function is different than uploading a file with chunks.
    /// See also [`create_upload_session()`] and [`OneDriveWriter::write_chunked`].
    pub async fn onedrive_upload_simple(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let url = format!(
            "{}:/content?@microsoft.graph.conflictBehavior={}&{}",
            self.onedrive_item_url(path, true),
            REPLACE_EXISTING_ITEM_WHEN_CONFLICT,
            GENERAL_SELECT_PARAM
        );

        // OneDrive upload API documentation requires "text/plain" as the content type.
        // In practice, OneDrive ignores the content type,
        // but decides the type (when stating) based on the extension name.
        // Also, when the extension name is unknown to OneDrive,
        // OneDrive sets the content type as "application/octet-stream".
        // We keep the content type according to the documentation.
        let mut request = Request::put(&url)
            .header(header::CONTENT_LENGTH, body.len())
            .header(header::CONTENT_TYPE, "text/plain");

        // when creating a new file, `IF-Match` has no effect.
        // when updating a file with the `If-Match`, and if the ETag mismatched,
        // OneDrive will return 412 Precondition Failed
        if let Some(if_match) = args.if_match() {
            request = request.header(header::IF_MATCH, if_match);
        }

        let mut request = request
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadContent"))
            .body(body)
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().send(request).await
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn onedrive_chunked_upload(
        &self,
        ctx: &OperationContext,
        url: &str,
        args: &OpWrite,
        offset: usize,
        chunk_end: usize,
        total_len: usize,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let mut request = Request::put(url);

        let range = format!("bytes {offset}-{chunk_end}/{total_len}");
        request = request.header(header::CONTENT_RANGE, range);

        let size = chunk_end - offset + 1;
        request = request.header(header::CONTENT_LENGTH, size);

        if let Some(mime) = args.content_type() {
            request = request.header(header::CONTENT_TYPE, mime)
        }

        let request = request
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadFragment"))
            .body(body)
            .map_err(new_request_build_error)?;
        // OneDrive documentation requires not sending the `Authorization` header

        ctx.http_transport().send(request).await
    }

    /// Create a upload session for chunk uploads
    ///
    /// This endpoint supports `If-None-Match` but [`onedrive_upload_simple()`] doesn't.
    ///
    /// Read more at https://learn.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_createuploadsession?view=odsp-graph-online#upload-bytes-to-the-upload-session
    pub(crate) async fn onedrive_create_upload_session(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let file_name = get_basename(path);
        let url = format!(
            "{}:/createUploadSession",
            self.onedrive_item_url(path, true),
        );
        let mut request = Request::post(url).header(header::CONTENT_TYPE, "application/json");

        if let Some(if_match) = args.if_match() {
            request = request.header(header::IF_MATCH, if_match);
        }

        let body = OneDriveUploadSessionCreationRequestBody::new(file_name.to_string());
        let body_bytes = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;
        let body = Buffer::from(Bytes::from(body_bytes));
        let mut request = request
            .extension(Operation::Write)
            .extension(ServiceOperation("CreateUploadSession"))
            .body(body)
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().send(request).await
    }

    /// Create a directory
    ///
    /// When creating a folder, OneDrive returns a status code with 201.
    /// When using `microsoft.graph.conflictBehavior=replace` to replace a folder, OneDrive returns 200.
    ///
    /// * `path` - the path to the folder without the root
    pub(crate) async fn onedrive_create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let parent_path = get_parent(path);
        let basename = get_basename(path);
        let folder_name = basename.strip_suffix('/').unwrap_or(basename);

        let url = format!(
            "{}:/children?{}",
            self.onedrive_item_url(parent_path, true),
            GENERAL_SELECT_PARAM
        );

        let payload = CreateDirPayload::new(folder_name.to_string());
        let body_bytes = serde_json::to_vec(&payload).map_err(new_json_serialize_error)?;
        let body = Buffer::from(bytes::Bytes::from(body_bytes));

        let mut request = Request::post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("CreateFolder"))
            .body(body)
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().send(request).await
    }

    /// Delete a `DriveItem`
    ///
    /// This moves the items to the recycle bin.
    pub(crate) async fn onedrive_delete(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let url = self.onedrive_item_url(path, true);

        let mut request = Request::delete(&url)
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteItem"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        ctx.http_transport().send(request).await
    }

    /// Initialize a copy
    ///
    /// * `source` - the path to the source folder without the root
    /// * `destination` - the path to the destination folder without the root
    ///
    /// See also: [`wait_until_complete()`]
    pub(crate) async fn initialize_copy(
        &self,
        ctx: &OperationContext,
        source: &str,
        destination: &str,
    ) -> Result<String> {
        // we must validate if source exist
        let response = self.onedrive_get_stat_plain(ctx, source).await?;
        if !response.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetItem")),
                response,
            ));
        }

        // We need to stat the destination parent folder to get a parent reference
        let destination_parent = get_parent(destination).to_string();
        let basename = get_basename(destination);

        let item = self.ensure_directory(ctx, &destination_parent).await?;
        let body = OneDrivePatchRequestBody {
            parent_reference: ParentReference {
                path: "".to_string(), // irrelevant for copy
                drive_id: item.parent_reference.drive_id,
                id: item.id,
            },
            name: basename.to_string(),
        };

        // ensure the destination file or folder doesn't exist
        let response = self.onedrive_get_stat_plain(ctx, destination).await?;
        match response.status() {
            // We must remove the file or folder because
            // OneDrive doesn't support `conflictBehavior` for the consumer OneDrive.
            // `conflictBehavior` seems to work for the consumer OneDrive sometimes could be a coincidence.
            // Read more at https://learn.microsoft.com/en-us/graph/api/driveitem-copy
            StatusCode::OK => {
                let response = self.onedrive_delete(ctx, destination).await?;
                match response.status() {
                    StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => {} // expected, intentionally empty
                    _ => {
                        return Err(parse_error(
                            ErrorContext::new(ServiceOperation("DeleteItem")),
                            response,
                        ));
                    }
                }
            }
            StatusCode::NOT_FOUND => {} // expected, intentionally empty
            _ => {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("GetItem")),
                    response,
                ));
            }
        }

        let url: String = format!("{}:/copy", self.onedrive_item_url(source, true));

        let body_bytes = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;
        let buffer = Buffer::from(Bytes::from(body_bytes));
        let mut request = Request::post(&url)
            .header(header::CONTENT_TYPE, "application/json")
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyItem"))
            .body(buffer)
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        let response = ctx.http_transport().send(request).await?;
        match response.status() {
            StatusCode::ACCEPTED => parse_location(response.headers())?
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "OneDrive didn't return a location URL",
                    )
                })
                .map(String::from),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CopyItem")),
                response,
            )),
        }
    }

    pub(crate) async fn wait_until_complete(
        &self,
        ctx: &OperationContext,
        monitor_url: String,
    ) -> Result<()> {
        for _attempt in 0..MAX_MONITOR_ATTEMPT {
            let mut request = Request::get(monitor_url.to_string())
                .header(header::CONTENT_TYPE, "application/json")
                .extension(Operation::Copy)
                .extension(ServiceOperation("MonitorCopy"))
                .body(Buffer::new())
                .map_err(new_request_build_error)?;

            self.sign(ctx, &mut request).await?;

            let response = ctx.http_transport().send(request).await?;
            let status: OneDriveMonitorStatus =
                serde_json::from_reader(response.into_body().reader())
                    .map_err(new_json_deserialize_error)?;
            if status.status == "completed" {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_secs(MONITOR_WAIT_SECOND)).await;
        }

        Err(Error::new(
            ErrorKind::Unexpected,
            "Exceed monitoring timeout",
        ))
    }

    pub(crate) async fn onedrive_move(
        &self,
        ctx: &OperationContext,
        source: &str,
        destination: &str,
    ) -> Result<()> {
        // We must validate if the source folder exists.
        let response = self.onedrive_get_stat_plain(ctx, source).await?;
        if !response.status().is_success() {
            return Err(Error::new(ErrorKind::NotFound, "source not found"));
        }

        // We want a parent reference about the destination's parent, or the destination folder itself.
        let destination_parent = get_parent(destination).to_string();
        let basename = get_basename(destination);

        let item = self.ensure_directory(ctx, &destination_parent).await?;
        let body = OneDrivePatchRequestBody {
            parent_reference: ParentReference {
                path: "".to_string(), // irrelevant for update
                // reusing `ParentReference` for convenience. The API requires this value to be correct.
                drive_id: item.parent_reference.drive_id,
                id: item.id,
            },
            name: basename.to_string(),
        };
        let body_bytes = serde_json::to_vec(&body).map_err(new_json_serialize_error)?;
        let buffer = Buffer::from(Bytes::from(body_bytes));
        let url: String = format!(
            "{}?@microsoft.graph.conflictBehavior={}&$select=id",
            self.onedrive_item_url(source, true),
            REPLACE_EXISTING_ITEM_WHEN_CONFLICT
        );
        let mut request = Request::patch(&url)
            .header(header::CONTENT_TYPE, "application/json")
            .extension(Operation::Rename)
            .extension(ServiceOperation("MoveItem"))
            .body(buffer)
            .map_err(new_request_build_error)?;

        self.sign(ctx, &mut request).await?;

        let response = ctx.http_transport().send(request).await?;
        match response.status() {
            // can get etag, metadata, etc...
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("MoveItem")),
                response,
            )),
        }
    }
}

// keeps track of OAuth 2.0 tokens and refreshes the access token.
pub struct OneDriveSigner {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,

    pub access_token: String,
    pub expires_in: Timestamp,
}

// OneDrive is part of Graph API hence shares the same authentication and authorization processes.
// `common` applies to account types:
//
// - consumers
// - work and school account
//
// set to `common` for simplicity
const ONEDRIVE_REFRESH_TOKEN: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";

impl OneDriveSigner {
    pub fn new() -> Self {
        OneDriveSigner {
            client_id: "".to_string(),
            client_secret: "".to_string(),
            refresh_token: "".to_string(),
            access_token: "".to_string(),
            expires_in: Timestamp::MIN,
        }
    }

    async fn refresh_tokens(&mut self, ctx: &OperationContext) -> Result<()> {
        // OneDrive users must provide at least this required permission scope
        let encoded_payload = format!(
            "client_id={}&client_secret={}&scope=offline_access%20Files.ReadWrite&refresh_token={}&grant_type=refresh_token",
            percent_encode_path(self.client_id.as_str()),
            percent_encode_path(self.client_secret.as_str()),
            percent_encode_path(self.refresh_token.as_str())
        );
        let request = Request::post(ONEDRIVE_REFRESH_TOKEN)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Buffer::from(encoded_payload))
            .map_err(new_request_build_error)?;

        let response = ctx.http_transport().send(request).await?;
        match response.status() {
            StatusCode::OK => {
                let resp_body = response.into_body();
                let data: GraphOAuthRefreshTokenResponseBody =
                    serde_json::from_reader(resp_body.reader())
                        .map_err(new_json_deserialize_error)?;
                self.access_token = data.access_token;
                self.refresh_token = data.refresh_token;
                self.expires_in = Timestamp::now() + Duration::from_secs(data.expires_in)
                    - Duration::from_secs(120); // assumes 2 mins graceful transmission for implementation simplicity
                Ok(())
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("RefreshAccessToken")),
                response,
            )),
        }
    }

    /// Sign a request.
    pub async fn sign<T>(
        &mut self,
        ctx: &OperationContext,
        request: &mut Request<T>,
    ) -> Result<()> {
        if !self.access_token.is_empty() && self.expires_in > Timestamp::now() {
            let value = format!("Bearer {}", self.access_token)
                .parse()
                .expect("access_token must be valid header value");

            request.headers_mut().insert(header::AUTHORIZATION, value);
            return Ok(());
        }

        self.refresh_tokens(ctx).await?;

        let auth_header_content = format!("Bearer {}", self.access_token)
            .parse()
            .expect("Fetched access_token is invalid as a header value");

        request
            .headers_mut()
            .insert(header::AUTHORIZATION, auth_header_content);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures::stream;
    use http::StatusCode;
    use opendal_core::raw::oio::List;

    use super::super::lister::OneDriveLister;
    use super::*;

    const ROOT_STAT_RESPONSE: &str = r#"{"id":"0","name":"root","lastModifiedDateTime":"2026-01-01T00:00:00Z","eTag":"aTag","size":0,"parentReference":{"path":"","driveId":"d","id":"p"},"folder":{"childCount":1}}"#;
    const ROOT_CHILDREN_RESPONSE: &str = r#"{"value":[{"id":"1","name":"test.txt","lastModifiedDateTime":"2026-01-01T00:00:00Z","eTag":"aTag","size":5,"parentReference":{"path":"/drive/root:","driveId":"d","id":"p"},"file":{"mimeType":"text/plain"}}]}"#;

    #[derive(Clone)]
    struct MockHttpTransport;

    impl HttpTransport for MockHttpTransport {
        async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
            let url = req.uri().to_string();
            let root_url = OneDriveCore::DRIVE_ROOT_URL;

            let (status, body) = if url == format!("{root_url}/children?{GENERAL_SELECT_PARAM}") {
                (StatusCode::OK, ROOT_CHILDREN_RESPONSE)
            } else if url == root_url {
                (StatusCode::OK, ROOT_STAT_RESPONSE)
            } else {
                (
                    StatusCode::NOT_FOUND,
                    r#"{"error":{"code":"itemNotFound","message":"Item not found"}}"#,
                )
            };

            let data = Bytes::from_static(body.as_bytes());
            let size = data.len() as u64;
            Ok(Response::builder()
                .status(status)
                .header(header::CONTENT_LENGTH, size)
                .body(HttpBody::new(
                    stream::iter(vec![Ok(Buffer::from(data))]),
                    Some(size),
                ))
                .unwrap())
        }
    }

    fn test_ctx() -> OperationContext {
        OperationContext::new().with_http_transport(HttpTransporter::new(MockHttpTransport))
    }

    fn test_core(root: &str) -> Arc<OneDriveCore> {
        let info = ServiceInfo::new("onedrive", root, "");

        let mut signer = OneDriveSigner::new();
        signer.access_token = "token".to_string();
        signer.expires_in = Timestamp::MAX;

        Arc::new(OneDriveCore {
            info,
            capability: Capability::default(),
            root: root.to_string(),
            signer: Arc::new(Mutex::new(signer)),
        })
    }

    #[test]
    fn list_request_for_root_targets_drive_root_children() {
        let core = test_core("/");
        let request = core.onedrive_list_request("/", None).unwrap();
        assert_eq!(
            request.uri().to_string(),
            format!(
                "{}/children?{}",
                OneDriveCore::DRIVE_ROOT_URL,
                GENERAL_SELECT_PARAM
            )
        );
    }

    #[test]
    fn list_request_for_nested_path_uses_path_addressing() {
        let core = test_core("/");
        let request = core.onedrive_list_request("foo/", Some(10)).unwrap();
        assert_eq!(
            request.uri().to_string(),
            format!(
                "{}:/foo:/children?{}&$top=10",
                OneDriveCore::DRIVE_ROOT_URL,
                GENERAL_SELECT_PARAM
            )
        );
    }

    #[test]
    fn list_request_for_root_under_custom_root_uses_path_addressing() {
        let core = test_core("/base/");
        let request = core.onedrive_list_request("", None).unwrap();
        assert_eq!(
            request.uri().to_string(),
            format!(
                "{}:/base:/children?{}",
                OneDriveCore::DRIVE_ROOT_URL,
                GENERAL_SELECT_PARAM
            )
        );
    }

    #[tokio::test]
    async fn list_root_returns_entries() {
        let core = test_core("/");
        let ctx = test_ctx();
        let lister = OneDriveLister::new("/".to_string(), core, ctx, &OpList::default());
        let mut lister = oio::PageLister::new(lister);

        let mut entries = Vec::new();
        while let Some(entry) = lister.next().await.unwrap() {
            entries.push(entry);
        }

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].mode(), EntryMode::DIR);
        assert_eq!(entries[1].path(), "test.txt");
        assert_eq!(entries[1].mode(), EntryMode::FILE);
    }

    #[test]
    fn invalid_request_is_temporary() {
        let response = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Buffer::from(r#"{"error":{"code":"invalidRequest"}}"#))
            .unwrap();

        let err = parse_error(
            ErrorContext::new(ServiceOperation("CreateFolder")),
            response,
        );

        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(err.is_temporary());
    }

    #[test]
    fn conflict_classification_uses_native_code() {
        let parse_conflict = |code: &str, operation| {
            let body = Buffer::from(format!(r#"{{"error":{{"code":"{code}"}}}}"#));
            let response = Response::builder()
                .status(StatusCode::CONFLICT)
                .body(body)
                .expect("response must build");
            parse_error(ErrorContext::new(operation), response)
        };

        let concurrent = parse_conflict(
            "Directory_ConcurrencyViolation",
            ServiceOperation("CreateFolder"),
        );
        assert_eq!(concurrent.kind(), ErrorKind::Conflict);
        assert!(concurrent.is_temporary());

        let name_conflict = parse_conflict("nameAlreadyExists", ServiceOperation("UploadFragment"));
        assert_eq!(name_conflict.kind(), ErrorKind::Conflict);
        assert!(!name_conflict.is_temporary());

        assert_eq!(
            parse_conflict("unknownConflict", ServiceOperation("MoveItem")).kind(),
            ErrorKind::Unexpected
        );
    }

    #[test]
    fn precondition_failed_requires_caller_condition() {
        let response = || {
            Response::builder()
                .status(StatusCode::PRECONDITION_FAILED)
                .body(Buffer::new())
                .expect("response must build")
        };
        let caller =
            ErrorContext::new(ServiceOperation("UploadContent")).with_caller_condition(true);
        assert_eq!(
            parse_error(caller, response()).kind(),
            ErrorKind::ConditionNotMatch
        );
        let no_condition = ErrorContext::new(ServiceOperation("UploadFragment"));
        assert_eq!(
            parse_error(no_condition, response()).kind(),
            ErrorKind::Unexpected
        );
    }
}

use crate::graph_model::GraphErrorResponse;

/// Context needed to classify an error from this service.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrorContext {
    service_operation: ServiceOperation,
    caller_condition: bool,
}

impl ErrorContext {
    pub(crate) const fn new(service_operation: ServiceOperation) -> Self {
        Self {
            service_operation,
            caller_condition: false,
        }
    }

    pub(crate) const fn with_caller_condition(mut self, caller_condition: bool) -> Self {
        self.caller_condition = caller_condition;
        self
    }
}

/// Parse an error response using its service request context.
pub(crate) fn parse_error(ctx: ErrorContext, response: Response<Buffer>) -> Error {
    let (parts, body) = response.into_parts();
    let bs = body.to_bytes();

    let graph_error = serde_json::from_slice::<GraphErrorResponse>(&bs).ok();
    let graph_code = graph_error
        .as_ref()
        .map(|response| response.error.code.as_str());
    let consistency_lag =
        parts.status == StatusCode::BAD_REQUEST && graph_code == Some("invalidRequest");

    let (kind, mut retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::CONFLICT if graph_code == Some("Directory_ConcurrencyViolation") => {
            (ErrorKind::Conflict, true)
        }
        StatusCode::CONFLICT if graph_code == Some("nameAlreadyExists") => {
            (ErrorKind::Conflict, false)
        }
        StatusCode::CONFLICT => (ErrorKind::Unexpected, false),
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        StatusCode::NOT_MODIFIED | StatusCode::PRECONDITION_FAILED if ctx.caller_condition => {
            (ErrorKind::ConditionNotMatch, false)
        }
        _ => (ErrorKind::Unexpected, false),
    };

    if consistency_lag {
        retryable = true;
    }

    let message = String::from_utf8_lossy(&bs);

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}
