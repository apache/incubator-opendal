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
use std::fmt::Formatter;
use std::sync::Arc;

use bytes;
use bytes::Buf;
use bytes::Bytes;
use chrono::DateTime;
use chrono::Utc;
use http::header;
use http::Request;
use http::Response;
use http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub struct GdriveCore {
    pub root: String,

    pub client: HttpClient,

    pub signer: Arc<Mutex<GdriveSigner>>,

    /// Cache the mapping from path to file id
    pub path_cache: PathCacher<GdrivePathQuery>,
}

impl Debug for GdriveCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut de = f.debug_struct("GdriveCore");
        de.field("root", &self.root);
        de.finish()
    }
}

impl GdriveCore {
    pub async fn gdrive_stat(&self, path: &str) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);
        let file_id = self.path_cache.get(&path).await?.ok_or(Error::new(
            ErrorKind::NotFound,
            format!("path not found: {}", path),
        ))?;

        // The file metadata in the Google Drive API is very complex.
        // For now, we only need the file id, name, mime type and modified time.
        let mut req = Request::get(format!(
            "https://www.googleapis.com/drive/v3/files/{}?fields=id,name,mimeType,size,modifiedTime",
            file_id
        ))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        self.sign(&mut req).await?;

        self.client.send(req).await
    }

    pub async fn gdrive_get(&self, path: &str, range: BytesRange) -> Result<Response<HttpBody>> {
        let path = build_abs_path(&self.root, path);
        let path_id = self.path_cache.get(&path).await?.ok_or(Error::new(
            ErrorKind::NotFound,
            format!("path not found: {}", path),
        ))?;

        let url: String = format!(
            "https://www.googleapis.com/drive/v3/files/{}?alt=media",
            path_id
        );

        let mut req = Request::get(&url)
            .header(header::RANGE, range.to_header())
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        self.sign(&mut req).await?;

        self.client.fetch(req).await
    }

    pub async fn gdrive_list(
        &self,
        file_id: &str,
        page_size: i32,
        next_page_token: &str,
    ) -> Result<Response<Buffer>> {
        let q = format!("'{}' in parents and trashed = false", file_id);
        let mut url = format!(
            "https://www.googleapis.com/drive/v3/files?pageSize={}&q={}",
            page_size,
            percent_encode_path(&q)
        );
        if !next_page_token.is_empty() {
            url += &format!("&pageToken={next_page_token}");
        };

        let mut req = Request::get(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        self.sign(&mut req).await?;

        self.client.send(req).await
    }

    // Update with content and metadata
    pub async fn gdrive_patch_metadata_request(
        &self,
        source: &str,
        target: &str,
    ) -> Result<Response<Buffer>> {
        let source_file_id = self.path_cache.get(source).await?.ok_or(Error::new(
            ErrorKind::NotFound,
            format!("source path not found: {}", source),
        ))?;
        let source_parent = get_parent(source);
        let source_parent_id = self
            .path_cache
            .get(source_parent)
            .await?
            .expect("old parent must exist");

        let target_parent_id = self.path_cache.ensure_dir(get_parent(target)).await?;
        let target_file_name = get_basename(target);

        let metadata = &json!({
            "name": target_file_name,
            "removeParents": [source_parent_id],
            "addParents": [target_parent_id],
        });

        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}",
            source_file_id
        );
        let mut req = Request::patch(url)
            .body(Buffer::from(Bytes::from(metadata.to_string())))
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.client.send(req).await
    }

    pub async fn gdrive_trash(&self, file_id: &str) -> Result<Response<Buffer>> {
        let url = format!("https://www.googleapis.com/drive/v3/files/{}", file_id);

        let body = serde_json::to_vec(&json!({
            "trashed": true
        }))
        .map_err(new_json_serialize_error)?;

        let mut req = Request::patch(&url)
            .body(Buffer::from(Bytes::from(body)))
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.client.send(req).await
    }

    /// Create a file with the content.
    pub async fn gdrive_upload_simple_request(
        &self,
        path: &str,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let parent = self.path_cache.ensure_dir(get_parent(path)).await?;

        let url = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart";

        let file_name = get_basename(path);

        let metadata = serde_json::to_vec(&json!({
            "name": file_name,
            "parents": [parent],
        }))
        .map_err(new_json_serialize_error)?;

        let req = Request::post(url).header("X-Upload-Content-Length", size);

        let multipart = Multipart::new()
            .part(
                FormDataPart::new("metadata")
                    .header(
                        header::CONTENT_TYPE,
                        "application/json; charset=UTF-8".parse().unwrap(),
                    )
                    .content(metadata),
            )
            .part(
                FormDataPart::new("file")
                    .header(
                        header::CONTENT_TYPE,
                        "application/octet-stream".parse().unwrap(),
                    )
                    .content(body),
            );

        let mut req = multipart.apply(req)?;

        self.sign(&mut req).await?;

        self.client.send(req).await
    }

    /// Overwrite the file with the content.
    ///
    /// # Notes
    ///
    /// - The file id is required. Do not use this method to create a file.
    pub async fn gdrive_upload_overwrite_simple_request(
        &self,
        file_id: &str,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let url = format!(
            "https://www.googleapis.com/upload/drive/v3/files/{}?uploadType=media",
            file_id
        );

        let mut req = Request::patch(url)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header(header::CONTENT_LENGTH, size)
            .header("X-Upload-Content-Length", size)
            .body(body)
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.client.send(req).await
    }

    pub async fn sign<T>(&self, req: &mut Request<T>) -> Result<()> {
        let mut signer = self.signer.lock().await;
        signer.sign(req).await
    }
}

#[derive(Clone)]
pub struct GdriveSigner {
    pub client: HttpClient,

    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,

    pub access_token: String,
    pub expires_in: DateTime<Utc>,
}

impl GdriveSigner {
    /// Create a new signer.
    pub fn new(client: HttpClient) -> Self {
        GdriveSigner {
            client,

            client_id: "".to_string(),
            client_secret: "".to_string(),
            refresh_token: "".to_string(),
            access_token: "".to_string(),
            expires_in: DateTime::<Utc>::MIN_UTC,
        }
    }

    /// Sign a request.
    pub async fn sign<T>(&mut self, req: &mut Request<T>) -> Result<()> {
        if !self.access_token.is_empty() && self.expires_in > Utc::now() {
            let value = format!("Bearer {}", self.access_token)
                .parse()
                .expect("access token must be valid header value");

            req.headers_mut().insert(header::AUTHORIZATION, value);
            return Ok(());
        }

        let url = format!(
            "https://oauth2.googleapis.com/token?refresh_token={}&client_id={}&client_secret={}&grant_type=refresh_token",
            self.refresh_token, self.client_id, self.client_secret
        );

        {
            let req = Request::post(url)
                .header(header::CONTENT_LENGTH, 0)
                .body(Buffer::new())
                .map_err(new_request_build_error)?;

            let resp = self.client.send(req).await?;
            let status = resp.status();

            match status {
                StatusCode::OK => {
                    let resp_body = resp.into_body();
                    let token: GdriveTokenResponse = serde_json::from_reader(resp_body.reader())
                        .map_err(new_json_deserialize_error)?;
                    self.access_token.clone_from(&token.access_token);
                    self.expires_in = Utc::now()
                        + chrono::TimeDelta::try_seconds(token.expires_in)
                            .expect("expires_in must be valid seconds")
                        - chrono::TimeDelta::try_seconds(120).expect("120 must be valid seconds");
                }
                _ => {
                    return Err(parse_error(resp));
                }
            }
        }

        let auth_header_content = format!("Bearer {}", self.access_token);
        req.headers_mut()
            .insert(header::AUTHORIZATION, auth_header_content.parse().unwrap());

        Ok(())
    }
}

pub struct GdrivePathQuery {
    pub client: HttpClient,
    pub signer: Arc<Mutex<GdriveSigner>>,
}

impl GdrivePathQuery {
    pub fn new(client: HttpClient, signer: Arc<Mutex<GdriveSigner>>) -> Self {
        GdrivePathQuery { client, signer }
    }
}

impl PathQuery for GdrivePathQuery {
    async fn root(&self) -> Result<String> {
        Ok("root".to_string())
    }

    async fn query(&self, parent_id: &str, name: &str) -> Result<Option<String>> {
        let mut queries = vec![
            // Make sure name has been replaced with escaped name.
            //
            // ref: <https://developers.google.com/drive/api/guides/ref-search-terms>
            format!(
                "name = '{}'",
                gdrive_normalize_dir_path(name.to_owned()).replace('\'', "\\'")
            ),
            format!("'{}' in parents", parent_id),
            "trashed = false".to_string(),
        ];
        if name.ends_with('/') {
            queries.push("mimeType = 'application/vnd.google-apps.folder'".to_string());
        }
        let query = queries.join(" and ");

        let url = format!(
            "https://www.googleapis.com/drive/v3/files?q={}",
            percent_encode_path(query.as_str())
        );

        let mut req = Request::get(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.signer.lock().await.sign(&mut req).await?;

        let resp = self.client.send(req).await?;
        let status = resp.status();

        match status {
            StatusCode::OK => {
                let body = resp.into_body();
                let meta: GdriveFileList =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

                if let Some(f) = meta.files.first() {
                    Ok(Some(f.id.clone()))
                } else {
                    Ok(None)
                }
            }
            _ => Err(parse_error(resp)),
        }
    }

    async fn create_dir(&self, parent_id: &str, name: &str) -> Result<String> {
        let url = "https://www.googleapis.com/drive/v3/files";

        let content = serde_json::to_vec(&json!({
            "name": gdrive_normalize_dir_path(name.to_owned()),
            "mimeType": "application/vnd.google-apps.folder",
            // If the parent is not provided, the folder will be created in the root folder.
            "parents": [parent_id],
        }))
        .map_err(new_json_serialize_error)?;

        let mut req = Request::post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Buffer::from(Bytes::from(content)))
            .map_err(new_request_build_error)?;

        self.signer.lock().await.sign(&mut req).await?;

        let resp = self.client.send(req).await?;
        if !resp.status().is_success() {
            return Err(parse_error(resp));
        }

        let body = resp.into_body();
        let file: GdriveFile =
            serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;
        Ok(file.id)
    }
}

#[derive(Deserialize)]
pub struct GdriveTokenResponse {
    access_token: String,
    expires_in: i64,
}

/// This is the file struct returned by the Google Drive API.
/// This is a complex struct, but we only add the fields we need.
/// refer to https://developers.google.com/drive/api/reference/rest/v3/files#File
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GdriveFile {
    pub mime_type: String,
    pub id: String,
    pub name: String,
    pub size: Option<String>,
    // The modified time is not returned unless the `fields`
    // query parameter contains `modifiedTime`.
    // As we only need the modified time when we do `stat` operation,
    // if other operations(such as search) do not specify the `fields` query parameter,
    // try to access this field, it will be `None`.
    pub modified_time: Option<String>,
}

/// refer to https://developers.google.com/drive/api/reference/rest/v3/files/list
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GdriveFileList {
    pub(crate) files: Vec<GdriveFile>,
    pub(crate) next_page_token: Option<String>,
}

/// On Google Drive, dir path name cannot end with slash (/) because if we add a slash, it would be different from the dir name
pub(super) fn gdrive_normalize_dir_path(path: String) -> String {
    if path == "/" {
        path
    } else if path.ends_with('/') {
        path.split_at(path.len() - 1).0.to_owned()
    } else {
        path
    }
}

#[test]
fn test_normalize_dir_path() {
    assert_eq!(gdrive_normalize_dir_path("/".to_owned()), "/");
    assert_eq!(gdrive_normalize_dir_path("/a".to_owned()), "/a");
    assert_eq!(gdrive_normalize_dir_path("/a/".to_owned()), "/a");
    assert_eq!(gdrive_normalize_dir_path("/a/b".to_owned()), "/a/b");
    assert_eq!(gdrive_normalize_dir_path("/a/b/".to_owned()), "/a/b");
}
