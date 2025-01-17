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
use std::fmt::Formatter;
use std::fmt::Write;
use std::time::Duration;

use backon::ExponentialBuilder;
use backon::Retryable;
use bytes::Bytes;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::HOST;
use http::header::IF_MATCH;
use http::header::IF_MODIFIED_SINCE;
use http::header::IF_NONE_MATCH;
use http::header::IF_UNMODIFIED_SINCE;
use http::Request;
use http::Response;
use once_cell::sync::Lazy;
use reqsign::GoogleCredential;
use reqsign::GoogleCredentialLoader;
use reqsign::GoogleSigner;
use reqsign::GoogleToken;
use reqsign::GoogleTokenLoader;
use serde::Deserialize;
use serde::Serialize;

use super::uri::percent_encode_path;
use crate::raw::*;
use crate::*;
use constants::*;

pub mod constants {
    pub const X_GOOG_ACL: &str = "x-goog-acl";
    pub const X_GOOG_STORAGE_CLASS: &str = "x-goog-storage-class";
    pub const X_GOOG_META_PREFIX: &str = "x-goog-meta-";
    pub const X_GOOG_IF_GENERATION_MATCH: &str = "x-goog-if-generation-match";
    pub const GENERATION: &str = "generation";
}

pub struct GcsCore {
    pub endpoint: String,
    pub bucket: String,
    pub root: String,

    pub client: HttpClient,
    pub signer: GoogleSigner,
    pub token_loader: GoogleTokenLoader,
    pub token: Option<String>,
    pub scope: String,
    pub credential_loader: GoogleCredentialLoader,

    pub predefined_acl: Option<String>,
    pub default_storage_class: Option<String>,

    pub allow_anonymous: bool,
    pub enable_versioning: bool,
}

impl Debug for GcsCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut de = f.debug_struct("Backend");
        de.field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

static BACKOFF: Lazy<ExponentialBuilder> =
    Lazy::new(|| ExponentialBuilder::default().with_jitter());

impl GcsCore {
    async fn load_token(&self) -> Result<Option<GoogleToken>> {
        if let Some(token) = &self.token {
            return Ok(Some(GoogleToken::new(token, usize::MAX, &self.scope)));
        }

        let cred = { || self.token_loader.load() }
            .retry(*BACKOFF)
            .await
            .map_err(new_request_credential_error)?;

        if let Some(cred) = cred {
            return Ok(Some(cred));
        }

        if self.allow_anonymous {
            return Ok(None);
        }

        Err(Error::new(
            ErrorKind::ConfigInvalid,
            "no valid credential found",
        ))
    }

    fn load_credential(&self) -> Result<Option<GoogleCredential>> {
        let cred = self
            .credential_loader
            .load()
            .map_err(new_request_credential_error)?;

        if let Some(cred) = cred {
            return Ok(Some(cred));
        }

        if self.allow_anonymous {
            return Ok(None);
        }

        Err(Error::new(
            ErrorKind::ConfigInvalid,
            "no valid credential found",
        ))
    }

    pub async fn sign<T>(&self, req: &mut Request<T>) -> Result<()> {
        if let Some(cred) = self.load_token().await? {
            self.signer
                .sign(req, &cred)
                .map_err(new_request_sign_error)?;
        } else {
            return Ok(());
        }

        // Always remove host header, let users' client to set it based on HTTP
        // version.
        //
        // As discussed in <https://github.com/seanmonstar/reqwest/issues/1809>,
        // google server could send RST_STREAM of PROTOCOL_ERROR if our request
        // contains host header.
        req.headers_mut().remove(HOST);

        Ok(())
    }

    pub fn sign_query<T>(&self, req: &mut Request<T>, duration: Duration) -> Result<()> {
        if let Some(cred) = self.load_credential()? {
            self.signer
                .sign_query(req, duration, &cred)
                .map_err(new_request_sign_error)?;
        } else {
            return Ok(());
        }

        // Always remove host header, let users' client to set it based on HTTP
        // version.
        //
        // As discussed in <https://github.com/seanmonstar/reqwest/issues/1809>,
        // google server could send RST_STREAM of PROTOCOL_ERROR if our request
        // contains host header.
        req.headers_mut().remove(HOST);

        Ok(())
    }

    #[inline]
    pub async fn send(&self, req: Request<Buffer>) -> Result<Response<Buffer>> {
        self.client.send(req).await
    }
}

impl GcsCore {
    pub fn gcs_get_object_request(
        &self,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/storage/v1/b/{}/o/{}?alt=media",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::GENERATION,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("&{}", query_args.join("&")));
        }

        let mut req = Request::get(&url);

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }
        if !range.is_full() {
            req = req.header(http::header::RANGE, range.to_header());
        }

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    // It's for presign operation. Gcs only supports query sign over XML API.
    pub fn gcs_get_object_xml_request(&self, path: &str, args: &OpRead) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}/{}", self.endpoint, self.bucket, p);

        let mut req = Request::get(&url);

        if let Some(version) = args.version() {
            req = req.header(constants::X_GOOG_IF_GENERATION_MATCH, version);
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }
        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_modified_since) = args.if_modified_since() {
            req = req.header(
                IF_MODIFIED_SINCE,
                format_datetime_into_http_date(if_modified_since),
            );
        }

        if let Some(if_unmodified_since) = args.if_unmodified_since() {
            req = req.header(
                IF_UNMODIFIED_SINCE,
                format_datetime_into_http_date(if_unmodified_since),
            );
        }

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn gcs_get_object(
        &self,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let mut req = self.gcs_get_object_request(path, range, args)?;

        self.sign(&mut req).await?;
        self.client.fetch(req).await
    }

    pub fn gcs_insert_object_request(
        &self,
        path: &str,
        size: Option<u64>,
        op: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let request_metadata = InsertRequestMetadata {
            storage_class: self.default_storage_class.as_deref(),
            cache_control: op.cache_control(),
            content_type: op.content_type(),
            metadata: op.user_metadata(),
        };

        let mut url = format!(
            "{}/upload/storage/v1/b/{}/o?uploadType={}&name={}",
            self.endpoint,
            self.bucket,
            if request_metadata.is_empty() {
                "media"
            } else {
                "multipart"
            },
            percent_encode_path(&p)
        );

        if let Some(acl) = &self.predefined_acl {
            write!(&mut url, "&predefinedAcl={}", acl).unwrap();
        }

        // Makes the operation conditional on whether the object's current generation
        // matches the given value. Setting to 0 makes the operation succeed only if
        // there are no live versions of the object.
        if op.if_not_exists() {
            write!(&mut url, "&ifGenerationMatch=0").unwrap();
        }

        let mut req = Request::post(&url);

        req = req.header(CONTENT_LENGTH, size.unwrap_or_default());

        if request_metadata.is_empty() {
            // If the metadata is empty, we do not set any `Content-Type` header,
            // since if we had it in the `op.content_type()`, it would be already set in the
            // `multipart` metadata body and this branch won't be executed.
            let req = req.body(body).map_err(new_request_build_error)?;
            Ok(req)
        } else {
            let mut multipart = Multipart::new();
            let metadata_part = FormDataPart::new("metadata")
                .header(
                    CONTENT_TYPE,
                    "application/json; charset=UTF-8".parse().unwrap(),
                )
                .content(
                    serde_json::to_vec(&request_metadata)
                        .expect("metadata serialization should success"),
                );
            multipart = multipart.part(metadata_part);

            let media_part = FormDataPart::new("media").content(body);
            multipart = multipart.part(media_part);

            let req = multipart.apply(Request::post(url))?;
            Ok(req)
        }
    }

    // It's for presign operation. Gcs only supports query sign over XML API.
    pub fn gcs_insert_object_xml_request(
        &self,
        path: &str,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}/{}", self.endpoint, self.bucket, p);

        let mut req = Request::put(&url);

        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("{X_GOOG_META_PREFIX}{key}"), value)
            }
        }

        if let Some(content_type) = args.content_type() {
            req = req.header(CONTENT_TYPE, content_type);
        }

        if let Some(acl) = &self.predefined_acl {
            req = req.header(X_GOOG_ACL, acl);
        }

        if let Some(storage_class) = &self.default_storage_class {
            req = req.header(X_GOOG_STORAGE_CLASS, storage_class);
        }

        let req = req.body(body).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub fn gcs_head_object_request(&self, path: &str, args: &OpStat) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/storage/v1/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::GENERATION,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::get(&url);

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    // It's for presign operation. Gcs only supports query sign over XML API.
    pub fn gcs_head_object_xml_request(
        &self,
        path: &str,
        args: &OpStat,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}/{}/{}", self.endpoint, self.bucket, p);

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::GENERATION,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::head(&url);

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn gcs_get_object_metadata(
        &self,
        path: &str,
        args: &OpStat,
    ) -> Result<Response<Buffer>> {
        let mut req = self.gcs_head_object_request(path, args)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn gcs_delete_object(&self, path: &str, args: OpDelete) -> Result<Response<Buffer>> {
        let mut req = self.gcs_delete_object_request(path, args)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub fn gcs_delete_object_request(&self, path: &str, args: OpDelete) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/storage/v1/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );

        let mut query_args = Vec::new();
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::GENERATION,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        Request::delete(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)
    }

    pub async fn gcs_delete_objects(
        &self,
        batch: Vec<(String, OpDelete)>,
    ) -> Result<Response<Buffer>> {
        let uri = format!("{}/batch/storage/v1", self.endpoint);

        let mut multipart = Multipart::new();

        for (idx, (path, args)) in batch.iter().enumerate() {
            let req = self.gcs_delete_object_request(path, args.clone())?;

            multipart = multipart.part(
                MixedPart::from_request(req).part_header("content-id".parse().unwrap(), idx.into()),
            );
        }

        let req = Request::post(uri);
        let mut req = multipart.apply(req)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub async fn gcs_copy_object(&self, from: &str, to: &str) -> Result<Response<Buffer>> {
        let source = build_abs_path(&self.root, from);
        let dest = build_abs_path(&self.root, to);

        let req_uri = format!(
            "{}/storage/v1/b/{}/o/{}/copyTo/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&source),
            self.bucket,
            percent_encode_path(&dest)
        );

        let mut req = Request::post(req_uri)
            .header(CONTENT_LENGTH, 0)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub async fn gcs_list_objects(
        &self,
        path: &str,
        page_token: &str,
        delimiter: &str,
        limit: Option<usize>,
        start_after: Option<String>,
        versions: bool,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/storage/v1/b/{}/o?prefix={}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );
        if versions {
            write!(url, "&versions=true").expect("write into string must succeed");
        }
        if !delimiter.is_empty() {
            write!(url, "&delimiter={delimiter}").expect("write into string must succeed");
        }
        if let Some(limit) = limit {
            write!(url, "&maxResults={limit}").expect("write into string must succeed");
        }
        if let Some(start_after) = start_after {
            let start_after = build_abs_path(&self.root, &start_after);
            write!(url, "&startOffset={}", percent_encode_path(&start_after))
                .expect("write into string must succeed");
        }

        if !page_token.is_empty() {
            // NOTE:
            //
            // GCS uses pageToken in request and nextPageToken in response
            //
            // Don't know how will those tokens be like so this part are copied
            // directly from AWS S3 service.
            write!(url, "&pageToken={}", percent_encode_path(page_token))
                .expect("write into string must succeed");
        }

        let mut req = Request::get(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn gcs_initiate_multipart_upload(&self, path: &str) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}/{}?uploads", self.endpoint, self.bucket, p);

        let mut req = Request::post(&url)
            .header(CONTENT_LENGTH, 0)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub async fn gcs_upload_part(
        &self,
        path: &str,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}?partNumber={}&uploadId={}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p),
            part_number,
            percent_encode_path(upload_id)
        );

        let mut req = Request::put(&url);

        req = req.header(CONTENT_LENGTH, size);

        let mut req = req.body(body).map_err(new_request_build_error)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub async fn gcs_complete_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
        parts: Vec<CompleteMultipartUploadRequestPart>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}?uploadId={}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p),
            percent_encode_path(upload_id)
        );

        let req = Request::post(&url);

        let content = quick_xml::se::to_string(&CompleteMultipartUploadRequest { part: parts })
            .map_err(new_xml_deserialize_error)?;
        // Make sure content length has been set to avoid post with chunked encoding.
        let req = req.header(CONTENT_LENGTH, content.len());
        // Set content-type to `application/xml` to avoid mixed with form post.
        let req = req.header(CONTENT_TYPE, "application/xml");

        let mut req = req
            .body(Buffer::from(Bytes::from(content)))
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub async fn gcs_abort_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}?uploadId={}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p),
            percent_encode_path(upload_id)
        );

        let mut req = Request::delete(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        self.sign(&mut req).await?;
        self.send(req).await
    }
}

#[derive(Debug, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InsertRequestMetadata<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    content_type: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_class: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<&'a HashMap<String, String>>,
}

impl InsertRequestMetadata<'_> {
    pub fn is_empty(&self) -> bool {
        self.content_type.is_none()
            && self.storage_class.is_none()
            && self.cache_control.is_none()
            && self.metadata.is_none()
    }
}
/// Response JSON from GCS list objects API.
///
/// refer to https://cloud.google.com/storage/docs/json_api/v1/objects/list for details
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListResponse {
    /// The continuation token.
    ///
    /// If this is the last page of results, then no continuation token is returned.
    pub next_page_token: Option<String>,
    /// Object name prefixes for objects that matched the listing request
    /// but were excluded from [items] because of a delimiter.
    pub prefixes: Vec<String>,
    /// The list of objects, ordered lexicographically by name.
    pub items: Vec<ListResponseItem>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListResponseItem {
    pub name: String,
    pub size: String,
    // metadata
    pub etag: String,
    pub md5_hash: String,
    pub updated: String,
    pub content_type: String,
    pub time_deleted: Option<String>,
}

/// Result of CreateMultipartUpload
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct InitiateMultipartUploadResult {
    pub upload_id: String,
}

/// Request of CompleteMultipartUploadRequest
#[derive(Default, Debug, Serialize)]
#[serde(default, rename = "CompleteMultipartUpload", rename_all = "PascalCase")]
pub struct CompleteMultipartUploadRequest {
    pub part: Vec<CompleteMultipartUploadRequestPart>,
}

#[derive(Clone, Default, Debug, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct CompleteMultipartUploadRequestPart {
    #[serde(rename = "PartNumber")]
    pub part_number: usize,
    #[serde(rename = "ETag")]
    pub etag: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_list_response() {
        let content = r#"
    {
  "kind": "storage#objects",
  "prefixes": [
    "dir/",
    "test/"
  ],
  "items": [
    {
      "kind": "storage#object",
      "id": "example/1.png/1660563214863653",
      "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/1.png",
      "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/1.png?generation=1660563214863653&alt=media",
      "name": "1.png",
      "bucket": "example",
      "generation": "1660563214863653",
      "metageneration": "1",
      "contentType": "image/png",
      "storageClass": "STANDARD",
      "size": "56535",
      "md5Hash": "fHcEH1vPwA6eTPqxuasXcg==",
      "crc32c": "j/un9g==",
      "etag": "CKWasoTgyPkCEAE=",
      "timeCreated": "2022-08-15T11:33:34.866Z",
      "updated": "2022-08-15T11:33:34.866Z",
      "timeStorageClassUpdated": "2022-08-15T11:33:34.866Z"
    },
    {
      "kind": "storage#object",
      "id": "example/2.png/1660563214883337",
      "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/2.png",
      "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/2.png?generation=1660563214883337&alt=media",
      "name": "2.png",
      "bucket": "example",
      "generation": "1660563214883337",
      "metageneration": "1",
      "contentType": "image/png",
      "storageClass": "STANDARD",
      "size": "45506",
      "md5Hash": "e6LsGusU7pFJZk+114NV1g==",
      "crc32c": "L00QAg==",
      "etag": "CIm0s4TgyPkCEAE=",
      "timeCreated": "2022-08-15T11:33:34.886Z",
      "updated": "2022-08-15T11:33:34.886Z",
      "timeStorageClassUpdated": "2022-08-15T11:33:34.886Z"
    }
  ]
}
    "#;

        let output: ListResponse =
            serde_json::from_str(content).expect("JSON deserialize must succeed");
        assert!(output.next_page_token.is_none());
        assert_eq!(output.items.len(), 2);
        assert_eq!(output.items[0].name, "1.png");
        assert_eq!(output.items[0].size, "56535");
        assert_eq!(output.items[0].md5_hash, "fHcEH1vPwA6eTPqxuasXcg==");
        assert_eq!(output.items[0].etag, "CKWasoTgyPkCEAE=");
        assert_eq!(output.items[0].updated, "2022-08-15T11:33:34.866Z");
        assert_eq!(output.items[1].name, "2.png");
        assert_eq!(output.items[1].size, "45506");
        assert_eq!(output.items[1].md5_hash, "e6LsGusU7pFJZk+114NV1g==");
        assert_eq!(output.items[1].etag, "CIm0s4TgyPkCEAE=");
        assert_eq!(output.items[1].updated, "2022-08-15T11:33:34.886Z");
        assert_eq!(output.items[1].content_type, "image/png");
        assert_eq!(output.prefixes, vec!["dir/", "test/"])
    }

    #[test]
    fn test_deserialize_list_response_with_next_page_token() {
        let content = r#"
    {
  "kind": "storage#objects",
  "prefixes": [
    "dir/",
    "test/"
  ],
  "nextPageToken": "CgYxMC5wbmc=",
  "items": [
    {
      "kind": "storage#object",
      "id": "example/1.png/1660563214863653",
      "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/1.png",
      "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/1.png?generation=1660563214863653&alt=media",
      "name": "1.png",
      "bucket": "example",
      "generation": "1660563214863653",
      "metageneration": "1",
      "contentType": "image/png",
      "storageClass": "STANDARD",
      "size": "56535",
      "md5Hash": "fHcEH1vPwA6eTPqxuasXcg==",
      "crc32c": "j/un9g==",
      "etag": "CKWasoTgyPkCEAE=",
      "timeCreated": "2022-08-15T11:33:34.866Z",
      "updated": "2022-08-15T11:33:34.866Z",
      "timeStorageClassUpdated": "2022-08-15T11:33:34.866Z"
    },
    {
      "kind": "storage#object",
      "id": "example/2.png/1660563214883337",
      "selfLink": "https://www.googleapis.com/storage/v1/b/example/o/2.png",
      "mediaLink": "https://content-storage.googleapis.com/download/storage/v1/b/example/o/2.png?generation=1660563214883337&alt=media",
      "name": "2.png",
      "bucket": "example",
      "generation": "1660563214883337",
      "metageneration": "1",
      "contentType": "image/png",
      "storageClass": "STANDARD",
      "size": "45506",
      "md5Hash": "e6LsGusU7pFJZk+114NV1g==",
      "crc32c": "L00QAg==",
      "etag": "CIm0s4TgyPkCEAE=",
      "timeCreated": "2022-08-15T11:33:34.886Z",
      "updated": "2022-08-15T11:33:34.886Z",
      "timeStorageClassUpdated": "2022-08-15T11:33:34.886Z"
    }
  ]
}
    "#;

        let output: ListResponse =
            serde_json::from_str(content).expect("JSON deserialize must succeed");
        assert_eq!(output.next_page_token, Some("CgYxMC5wbmc=".to_string()));
        assert_eq!(output.items.len(), 2);
        assert_eq!(output.items[0].name, "1.png");
        assert_eq!(output.items[0].size, "56535");
        assert_eq!(output.items[0].md5_hash, "fHcEH1vPwA6eTPqxuasXcg==");
        assert_eq!(output.items[0].etag, "CKWasoTgyPkCEAE=");
        assert_eq!(output.items[0].updated, "2022-08-15T11:33:34.866Z");
        assert_eq!(output.items[1].name, "2.png");
        assert_eq!(output.items[1].size, "45506");
        assert_eq!(output.items[1].md5_hash, "e6LsGusU7pFJZk+114NV1g==");
        assert_eq!(output.items[1].etag, "CIm0s4TgyPkCEAE=");
        assert_eq!(output.items[1].updated, "2022-08-15T11:33:34.886Z");
        assert_eq!(output.prefixes, vec!["dir/", "test/"])
    }
}
