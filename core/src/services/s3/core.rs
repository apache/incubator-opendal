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

use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Write;
use std::sync::atomic;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use bytes::Bytes;
use constants::X_AMZ_META_PREFIX;
use http::header::CACHE_CONTROL;
use http::header::CONTENT_DISPOSITION;
use http::header::CONTENT_ENCODING;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::HOST;
use http::header::IF_MATCH;
use http::header::IF_NONE_MATCH;
use http::header::{HeaderName, IF_MODIFIED_SINCE, IF_UNMODIFIED_SINCE};
use http::HeaderValue;
use http::Request;
use http::Response;
use reqsign::AwsCredential;
use reqsign::AwsCredentialLoad;
use reqsign::AwsV4Signer;
use serde::Deserialize;
use serde::Serialize;

use crate::raw::*;
use crate::*;

pub mod constants {
    pub const X_AMZ_COPY_SOURCE: &str = "x-amz-copy-source";

    pub const X_AMZ_SERVER_SIDE_ENCRYPTION: &str = "x-amz-server-side-encryption";
    pub const X_AMZ_SERVER_SIDE_ENCRYPTION_CUSTOMER_ALGORITHM: &str =
        "x-amz-server-side-encryption-customer-algorithm";
    pub const X_AMZ_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY: &str =
        "x-amz-server-side-encryption-customer-key";
    pub const X_AMZ_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY_MD5: &str =
        "x-amz-server-side-encryption-customer-key-md5";
    pub const X_AMZ_SERVER_SIDE_ENCRYPTION_AWS_KMS_KEY_ID: &str =
        "x-amz-server-side-encryption-aws-kms-key-id";
    pub const X_AMZ_STORAGE_CLASS: &str = "x-amz-storage-class";

    pub const X_AMZ_COPY_SOURCE_SERVER_SIDE_ENCRYPTION_CUSTOMER_ALGORITHM: &str =
        "x-amz-copy-source-server-side-encryption-customer-algorithm";
    pub const X_AMZ_COPY_SOURCE_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY: &str =
        "x-amz-copy-source-server-side-encryption-customer-key";
    pub const X_AMZ_COPY_SOURCE_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY_MD5: &str =
        "x-amz-copy-source-server-side-encryption-customer-key-md5";

    pub const X_AMZ_META_PREFIX: &str = "x-amz-meta-";

    pub const X_AMZ_VERSION_ID: &str = "x-amz-version-id";
    pub const X_AMZ_OBJECT_SIZE: &str = "x-amz-object-size";

    pub const RESPONSE_CONTENT_DISPOSITION: &str = "response-content-disposition";
    pub const RESPONSE_CONTENT_TYPE: &str = "response-content-type";
    pub const RESPONSE_CACHE_CONTROL: &str = "response-cache-control";

    pub const S3_QUERY_VERSION_ID: &str = "versionId";
}

pub struct S3Core {
    pub bucket: String,
    pub endpoint: String,
    pub root: String,
    pub server_side_encryption: Option<HeaderValue>,
    pub server_side_encryption_aws_kms_key_id: Option<HeaderValue>,
    pub server_side_encryption_customer_algorithm: Option<HeaderValue>,
    pub server_side_encryption_customer_key: Option<HeaderValue>,
    pub server_side_encryption_customer_key_md5: Option<HeaderValue>,
    pub default_storage_class: Option<HeaderValue>,
    pub allow_anonymous: bool,
    pub disable_stat_with_override: bool,
    pub enable_versioning: bool,

    pub signer: AwsV4Signer,
    pub loader: Box<dyn AwsCredentialLoad>,
    pub credential_loaded: AtomicBool,
    pub client: HttpClient,
    pub delete_max_size: usize,
    pub checksum_algorithm: Option<ChecksumAlgorithm>,
    pub disable_write_with_if_match: bool,
}

impl Debug for S3Core {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Core")
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl S3Core {
    /// If credential is not found, we will not sign the request.
    async fn load_credential(&self) -> Result<Option<AwsCredential>> {
        let cred = self
            .loader
            .load_credential(GLOBAL_REQWEST_CLIENT.clone())
            .await
            .map_err(new_request_credential_error)?;

        if let Some(cred) = cred {
            // Update credential_loaded to true if we have load credential successfully.
            self.credential_loaded
                .store(true, atomic::Ordering::Relaxed);
            return Ok(Some(cred));
        }

        // If we have load credential before but failed to load this time, we should
        // return error instead.
        if self.credential_loaded.load(atomic::Ordering::Relaxed) {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                "credential was previously loaded successfully but has failed this time",
            )
            .set_temporary());
        }

        // Credential is empty and users allow anonymous access, we will not sign the request.
        if self.allow_anonymous {
            return Ok(None);
        }

        Err(Error::new(
            ErrorKind::PermissionDenied,
            "no valid credential found and anonymous access is not allowed",
        ))
    }

    pub async fn sign<T>(&self, req: &mut Request<T>) -> Result<()> {
        let cred = if let Some(cred) = self.load_credential().await? {
            cred
        } else {
            return Ok(());
        };

        self.signer
            .sign(req, &cred)
            .map_err(new_request_sign_error)?;

        // Always remove host header, let users' client to set it based on HTTP
        // version.
        //
        // As discussed in <https://github.com/seanmonstar/reqwest/issues/1809>,
        // google server could send RST_STREAM of PROTOCOL_ERROR if our request
        // contains host header.
        req.headers_mut().remove(HOST);

        Ok(())
    }

    pub async fn sign_query<T>(&self, req: &mut Request<T>, duration: Duration) -> Result<()> {
        let cred = if let Some(cred) = self.load_credential().await? {
            cred
        } else {
            return Ok(());
        };

        self.signer
            .sign_query(req, duration, &cred)
            .map_err(new_request_sign_error)?;

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

    /// # Note
    ///
    /// header like X_AMZ_SERVER_SIDE_ENCRYPTION doesn't need to set while
    /// get or stat.
    pub fn insert_sse_headers(
        &self,
        mut req: http::request::Builder,
        is_write: bool,
    ) -> http::request::Builder {
        if is_write {
            if let Some(v) = &self.server_side_encryption {
                let mut v = v.clone();
                v.set_sensitive(true);

                req = req.header(
                    HeaderName::from_static(constants::X_AMZ_SERVER_SIDE_ENCRYPTION),
                    v,
                )
            }
            if let Some(v) = &self.server_side_encryption_aws_kms_key_id {
                let mut v = v.clone();
                v.set_sensitive(true);

                req = req.header(
                    HeaderName::from_static(constants::X_AMZ_SERVER_SIDE_ENCRYPTION_AWS_KMS_KEY_ID),
                    v,
                )
            }
        }

        if let Some(v) = &self.server_side_encryption_customer_algorithm {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(
                HeaderName::from_static(constants::X_AMZ_SERVER_SIDE_ENCRYPTION_CUSTOMER_ALGORITHM),
                v,
            )
        }
        if let Some(v) = &self.server_side_encryption_customer_key {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(
                HeaderName::from_static(constants::X_AMZ_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY),
                v,
            )
        }
        if let Some(v) = &self.server_side_encryption_customer_key_md5 {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(
                HeaderName::from_static(constants::X_AMZ_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY_MD5),
                v,
            )
        }

        req
    }
    pub fn calculate_checksum(&self, body: &Buffer) -> Option<String> {
        match self.checksum_algorithm {
            None => None,
            Some(ChecksumAlgorithm::Crc32c) => {
                let mut crc = 0u32;
                body.clone()
                    .for_each(|b| crc = crc32c::crc32c_append(crc, &b));
                Some(BASE64_STANDARD.encode(crc.to_be_bytes()))
            }
        }
    }
    pub fn insert_checksum_header(
        &self,
        mut req: http::request::Builder,
        checksum: &str,
    ) -> http::request::Builder {
        if let Some(checksum_algorithm) = self.checksum_algorithm.as_ref() {
            req = req.header(checksum_algorithm.to_header_name(), checksum);
        }
        req
    }

    pub fn insert_checksum_type_header(
        &self,
        mut req: http::request::Builder,
    ) -> http::request::Builder {
        if let Some(checksum_algorithm) = self.checksum_algorithm.as_ref() {
            req = req.header("x-amz-checksum-algorithm", checksum_algorithm.to_string());
        }
        req
    }
}

impl S3Core {
    pub fn s3_head_object_request(&self, path: &str, args: OpStat) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        // Add query arguments to the URL based on response overrides
        let mut query_args = Vec::new();
        if let Some(override_content_disposition) = args.override_content_disposition() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CONTENT_DISPOSITION,
                percent_encode_path(override_content_disposition)
            ))
        }
        if let Some(override_content_type) = args.override_content_type() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CONTENT_TYPE,
                percent_encode_path(override_content_type)
            ))
        }
        if let Some(override_cache_control) = args.override_cache_control() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CACHE_CONTROL,
                percent_encode_path(override_cache_control)
            ))
        }
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::S3_QUERY_VERSION_ID,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::head(&url);

        req = self.insert_sse_headers(req, false);

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }
        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
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

    pub fn s3_get_object_request(
        &self,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        // Construct headers to add to the request
        let mut url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        // Add query arguments to the URL based on response overrides
        let mut query_args = Vec::new();
        if let Some(override_content_disposition) = args.override_content_disposition() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CONTENT_DISPOSITION,
                percent_encode_path(override_content_disposition)
            ))
        }
        if let Some(override_content_type) = args.override_content_type() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CONTENT_TYPE,
                percent_encode_path(override_content_type)
            ))
        }
        if let Some(override_cache_control) = args.override_cache_control() {
            query_args.push(format!(
                "{}={}",
                constants::RESPONSE_CACHE_CONTROL,
                percent_encode_path(override_cache_control)
            ))
        }
        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::S3_QUERY_VERSION_ID,
                percent_decode_path(version)
            ))
        }
        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::get(&url);

        if !range.is_full() {
            req = req.header(http::header::RANGE, range.to_header());
        }

        if let Some(if_none_match) = args.if_none_match() {
            req = req.header(IF_NONE_MATCH, if_none_match);
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
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

        // Set SSE headers.
        // TODO: how will this work with presign?
        req = self.insert_sse_headers(req, false);

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn s3_get_object(
        &self,
        path: &str,
        range: BytesRange,
        args: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let mut req = self.s3_get_object_request(path, range, args)?;

        self.sign(&mut req).await?;

        self.client.fetch(req).await
    }

    pub fn s3_put_object_request(
        &self,
        path: &str,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let mut req = Request::put(&url);

        if let Some(size) = size {
            req = req.header(CONTENT_LENGTH, size.to_string())
        }

        if let Some(mime) = args.content_type() {
            req = req.header(CONTENT_TYPE, mime)
        }

        if let Some(pos) = args.content_disposition() {
            req = req.header(CONTENT_DISPOSITION, pos)
        }

        if let Some(encoding) = args.content_encoding() {
            req = req.header(CONTENT_ENCODING, encoding);
        }

        if let Some(cache_control) = args.cache_control() {
            req = req.header(CACHE_CONTROL, cache_control)
        }

        if let Some(if_match) = args.if_match() {
            req = req.header(IF_MATCH, if_match);
        }

        if args.if_not_exists() {
            req = req.header(IF_NONE_MATCH, "*");
        }

        // Set storage class header
        if let Some(v) = &self.default_storage_class {
            req = req.header(HeaderName::from_static(constants::X_AMZ_STORAGE_CLASS), v);
        }

        // Set user metadata headers.
        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("{X_AMZ_META_PREFIX}{key}"), value)
            }
        }

        // Set SSE headers.
        req = self.insert_sse_headers(req, true);

        // Calculate Checksum.
        if let Some(checksum) = self.calculate_checksum(&body) {
            // Set Checksum header.
            req = self.insert_checksum_header(req, &checksum);
        }

        // Set body
        let req = req.body(body).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn s3_head_object(&self, path: &str, args: OpStat) -> Result<Response<Buffer>> {
        let mut req = self.s3_head_object_request(path, args)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn s3_delete_object(&self, path: &str, args: &OpDelete) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let mut query_args = Vec::new();

        if let Some(version) = args.version() {
            query_args.push(format!(
                "{}={}",
                constants::S3_QUERY_VERSION_ID,
                percent_encode_path(version)
            ))
        }

        if !query_args.is_empty() {
            url.push_str(&format!("?{}", query_args.join("&")));
        }

        let mut req = Request::delete(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn s3_copy_object(&self, from: &str, to: &str) -> Result<Response<Buffer>> {
        let from = build_abs_path(&self.root, from);
        let to = build_abs_path(&self.root, to);

        let source = format!("{}/{}", self.bucket, percent_encode_path(&from));
        let target = format!("{}/{}", self.endpoint, percent_encode_path(&to));

        let mut req = Request::put(&target);

        // Set SSE headers.
        req = self.insert_sse_headers(req, true);

        if let Some(v) = &self.server_side_encryption_customer_algorithm {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(
                HeaderName::from_static(
                    constants::X_AMZ_COPY_SOURCE_SERVER_SIDE_ENCRYPTION_CUSTOMER_ALGORITHM,
                ),
                v,
            )
        }

        if let Some(v) = &self.server_side_encryption_customer_key {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(
                HeaderName::from_static(
                    constants::X_AMZ_COPY_SOURCE_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY,
                ),
                v,
            )
        }

        if let Some(v) = &self.server_side_encryption_customer_key_md5 {
            let mut v = v.clone();
            v.set_sensitive(true);

            req = req.header(
                HeaderName::from_static(
                    constants::X_AMZ_COPY_SOURCE_SERVER_SIDE_ENCRYPTION_CUSTOMER_KEY_MD5,
                ),
                v,
            )
        }

        let mut req = req
            .header(constants::X_AMZ_COPY_SOURCE, &source)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn s3_list_objects(
        &self,
        path: &str,
        continuation_token: &str,
        delimiter: &str,
        limit: Option<usize>,
        start_after: Option<String>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}?list-type=2", self.endpoint);
        if !p.is_empty() {
            write!(url, "&prefix={}", percent_encode_path(&p))
                .expect("write into string must succeed");
        }
        if !delimiter.is_empty() {
            write!(url, "&delimiter={delimiter}").expect("write into string must succeed");
        }
        if let Some(limit) = limit {
            write!(url, "&max-keys={limit}").expect("write into string must succeed");
        }
        if let Some(start_after) = start_after {
            write!(url, "&start-after={}", percent_encode_path(&start_after))
                .expect("write into string must succeed");
        }
        if !continuation_token.is_empty() {
            // AWS S3 could return continuation-token that contains `=`
            // which could lead `reqsign` parse query wrongly.
            // URL encode continuation-token before starting signing so that
            // our signer will not be confused.
            write!(
                url,
                "&continuation-token={}",
                percent_encode_path(continuation_token)
            )
            .expect("write into string must succeed");
        }

        let mut req = Request::get(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn s3_initiate_multipart_upload(
        &self,
        path: &str,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}?uploads", self.endpoint, percent_encode_path(&p));

        let mut req = Request::post(&url);

        if let Some(mime) = args.content_type() {
            req = req.header(CONTENT_TYPE, mime)
        }

        if let Some(content_disposition) = args.content_disposition() {
            req = req.header(CONTENT_DISPOSITION, content_disposition)
        }

        if let Some(cache_control) = args.cache_control() {
            req = req.header(CACHE_CONTROL, cache_control)
        }

        // Set storage class header
        if let Some(v) = &self.default_storage_class {
            req = req.header(HeaderName::from_static(constants::X_AMZ_STORAGE_CLASS), v);
        }

        // Set user metadata headers.
        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("{X_AMZ_META_PREFIX}{key}"), value)
            }
        }

        // Set SSE headers.
        let req = self.insert_sse_headers(req, true);

        // Set SSE headers.
        let req = self.insert_checksum_type_header(req);

        let mut req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub fn s3_upload_part_request(
        &self,
        path: &str,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
        checksum: Option<String>,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}?partNumber={}&uploadId={}",
            self.endpoint,
            percent_encode_path(&p),
            part_number,
            percent_encode_path(upload_id)
        );

        let mut req = Request::put(&url);

        req = req.header(CONTENT_LENGTH, size);

        // Set SSE headers.
        req = self.insert_sse_headers(req, true);

        if let Some(checksum) = checksum {
            // Set Checksum header.
            req = self.insert_checksum_header(req, &checksum);
        }

        // Set body
        let req = req.body(body).map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn s3_complete_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
        parts: Vec<CompleteMultipartUploadRequestPart>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}?uploadId={}",
            self.endpoint,
            percent_encode_path(&p),
            percent_encode_path(upload_id)
        );

        let req = Request::post(&url);

        // Set SSE headers.
        let req = self.insert_sse_headers(req, true);

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

    /// Abort an on-going multipart upload.
    pub async fn s3_abort_multipart_upload(
        &self,
        path: &str,
        upload_id: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}?uploadId={}",
            self.endpoint,
            percent_encode_path(&p),
            percent_encode_path(upload_id)
        );

        let mut req = Request::delete(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub async fn s3_delete_objects(
        &self,
        paths: Vec<(String, OpDelete)>,
    ) -> Result<Response<Buffer>> {
        let url = format!("{}/?delete", self.endpoint);

        let req = Request::post(&url);

        let content = quick_xml::se::to_string(&DeleteObjectsRequest {
            object: paths
                .into_iter()
                .map(|(path, op)| DeleteObjectsRequestObject {
                    key: build_abs_path(&self.root, &path),
                    version_id: op.version().map(|v| v.to_owned()),
                })
                .collect(),
        })
        .map_err(new_xml_deserialize_error)?;

        // Make sure content length has been set to avoid post with chunked encoding.
        let req = req.header(CONTENT_LENGTH, content.len());
        // Set content-type to `application/xml` to avoid mixed with form post.
        let req = req.header(CONTENT_TYPE, "application/xml");
        // Set content-md5 as required by API.
        let req = req.header("CONTENT-MD5", format_content_md5(content.as_bytes()));

        let mut req = req
            .body(Buffer::from(Bytes::from(content)))
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn s3_list_object_versions(
        &self,
        prefix: &str,
        delimiter: &str,
        limit: Option<usize>,
        key_marker: &str,
        version_id_marker: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, prefix);

        let mut url = format!("{}?versions", self.endpoint);
        if !p.is_empty() {
            write!(url, "&prefix={}", percent_encode_path(p.as_str()))
                .expect("write into string must succeed");
        }
        if !delimiter.is_empty() {
            write!(url, "&delimiter={}", delimiter).expect("write into string must succeed");
        }

        if let Some(limit) = limit {
            write!(url, "&max-keys={}", limit).expect("write into string must succeed");
        }
        if !key_marker.is_empty() {
            write!(url, "&key-marker={}", percent_encode_path(key_marker))
                .expect("write into string must succeed");
        }
        if !version_id_marker.is_empty() {
            write!(
                url,
                "&version-id-marker={}",
                percent_encode_path(version_id_marker)
            )
            .expect("write into string must succeed");
        }

        let mut req = Request::get(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }
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
    /// # TODO
    ///
    /// quick-xml will do escape on `"` which leads to our serialized output is
    /// not the same as aws s3's example.
    ///
    /// Ideally, we could use `serialize_with` to address this (buf failed)
    ///
    /// ```ignore
    /// #[derive(Default, Debug, Serialize)]
    /// #[serde(default, rename_all = "PascalCase")]
    /// struct CompleteMultipartUploadRequestPart {
    ///     #[serde(rename = "PartNumber")]
    ///     part_number: usize,
    ///     #[serde(rename = "ETag", serialize_with = "partial_escape")]
    ///     etag: String,
    /// }
    ///
    /// fn partial_escape<S>(s: &str, ser: S) -> Result<S::Ok, S::Error>
    /// where
    ///     S: serde::Serializer,
    /// {
    ///     ser.serialize_str(&String::from_utf8_lossy(
    ///         &quick_xml::escape::partial_escape(s.as_bytes()),
    ///     ))
    /// }
    /// ```
    ///
    /// ref: <https://github.com/tafia/quick-xml/issues/362>
    #[serde(rename = "ETag")]
    pub etag: String,
    #[serde(rename = "ChecksumCRC32C", skip_serializing_if = "Option::is_none")]
    pub checksum_crc32c: Option<String>,
}

/// OutPut of `CompleteMultipartUpload` operation
#[derive(Debug, Default, Deserialize)]
#[serde[default, rename_all = "PascalCase"]]
pub struct CompleteMultipartUploadResult {
    pub bucket: String,
    pub key: String,
    pub location: String,
    #[serde(rename = "ETag")]
    pub etag: String,
}

/// Request of DeleteObjects.
#[derive(Default, Debug, Serialize)]
#[serde(default, rename = "Delete", rename_all = "PascalCase")]
pub struct DeleteObjectsRequest {
    pub object: Vec<DeleteObjectsRequestObject>,
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct DeleteObjectsRequestObject {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
}

/// Result of DeleteObjects.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename = "DeleteResult", rename_all = "PascalCase")]
pub struct DeleteObjectsResult {
    pub deleted: Vec<DeleteObjectsResultDeleted>,
    pub error: Vec<DeleteObjectsResultError>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DeleteObjectsResultDeleted {
    pub key: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct DeleteObjectsResultError {
    pub code: String,
    pub key: String,
    pub message: String,
}

/// Output of ListBucket/ListObjects.
///
/// ## Note
///
/// Use `Option` in `is_truncated` and `next_continuation_token` to make
/// the behavior more clear so that we can be compatible to more s3 services.
///
/// And enable `serde(default)` so that we can keep going even when some field
/// is not exist.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ListObjectsOutput {
    pub is_truncated: Option<bool>,
    pub next_continuation_token: Option<String>,
    pub common_prefixes: Vec<OutputCommonPrefix>,
    pub contents: Vec<ListObjectsOutputContent>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListObjectsOutputContent {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
    #[serde(rename = "ETag")]
    pub etag: Option<String>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OutputCommonPrefix {
    pub prefix: String,
}

/// Output of ListObjectVersions
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ListObjectVersionsOutput {
    pub is_truncated: Option<bool>,
    pub next_key_marker: Option<String>,
    pub next_version_id_marker: Option<String>,
    pub common_prefixes: Vec<OutputCommonPrefix>,
    pub version: Vec<ListObjectVersionsOutputVersion>,
    pub delete_marker: Vec<ListObjectVersionsOutputDeleteMarker>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListObjectVersionsOutputVersion {
    pub key: String,
    pub version_id: String,
    pub is_latest: bool,
    pub size: u64,
    pub last_modified: String,
    #[serde(rename = "ETag")]
    pub etag: Option<String>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ListObjectVersionsOutputDeleteMarker {
    pub key: String,
    pub version_id: String,
    pub is_latest: bool,
    pub last_modified: String,
}

pub enum ChecksumAlgorithm {
    Crc32c,
}
impl ChecksumAlgorithm {
    pub fn to_header_name(&self) -> HeaderName {
        match self {
            Self::Crc32c => HeaderName::from_static("x-amz-checksum-crc32c"),
        }
    }
}
impl Display for ChecksumAlgorithm {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Crc32c => "CRC32C",
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use bytes::Buf;
    use bytes::Bytes;

    use super::*;

    /// This example is from https://docs.aws.amazon.com/AmazonS3/latest/API/API_CreateMultipartUpload.html#API_CreateMultipartUpload_Examples
    #[test]
    fn test_deserialize_initiate_multipart_upload_result() {
        let bs = Bytes::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <InitiateMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
              <Bucket>example-bucket</Bucket>
              <Key>example-object</Key>
              <UploadId>VXBsb2FkIElEIGZvciA2aWWpbmcncyBteS1tb3ZpZS5tMnRzIHVwbG9hZA</UploadId>
            </InitiateMultipartUploadResult>"#,
        );

        let out: InitiateMultipartUploadResult =
            quick_xml::de::from_reader(bs.reader()).expect("must success");

        assert_eq!(
            out.upload_id,
            "VXBsb2FkIElEIGZvciA2aWWpbmcncyBteS1tb3ZpZS5tMnRzIHVwbG9hZA"
        )
    }

    /// This example is from https://docs.aws.amazon.com/AmazonS3/latest/API/API_CompleteMultipartUpload.html#API_CompleteMultipartUpload_Examples
    #[test]
    fn test_serialize_complete_multipart_upload_request() {
        let req = CompleteMultipartUploadRequest {
            part: vec![
                CompleteMultipartUploadRequestPart {
                    part_number: 1,
                    etag: "\"a54357aff0632cce46d942af68356b38\"".to_string(),
                    ..Default::default()
                },
                CompleteMultipartUploadRequestPart {
                    part_number: 2,
                    etag: "\"0c78aef83f66abc1fa1e8477f296d394\"".to_string(),
                    ..Default::default()
                },
                CompleteMultipartUploadRequestPart {
                    part_number: 3,
                    etag: "\"acbd18db4cc2f85cedef654fccc4a4d8\"".to_string(),
                    ..Default::default()
                },
            ],
        };

        let actual = quick_xml::se::to_string(&req).expect("must succeed");

        pretty_assertions::assert_eq!(
            actual,
            r#"<CompleteMultipartUpload>
             <Part>
                <PartNumber>1</PartNumber>
               <ETag>"a54357aff0632cce46d942af68356b38"</ETag>
             </Part>
             <Part>
                <PartNumber>2</PartNumber>
               <ETag>"0c78aef83f66abc1fa1e8477f296d394"</ETag>
             </Part>
             <Part>
               <PartNumber>3</PartNumber>
               <ETag>"acbd18db4cc2f85cedef654fccc4a4d8"</ETag>
             </Part>
            </CompleteMultipartUpload>"#
                // Cleanup space and new line
                .replace([' ', '\n'], "")
        )
    }

    /// this example is from: https://docs.aws.amazon.com/AmazonS3/latest/API/API_CompleteMultipartUpload.html
    #[test]
    fn test_deserialize_complete_multipart_upload_result() {
        let bs = Bytes::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <CompleteMultipartUploadResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
             <Location>http://Example-Bucket.s3.region.amazonaws.com/Example-Object</Location>
             <Bucket>Example-Bucket</Bucket>
             <Key>Example-Object</Key>
             <ETag>"3858f62230ac3c915f300c664312c11f-9"</ETag>
            </CompleteMultipartUploadResult>"#,
        );

        let out: CompleteMultipartUploadResult =
            quick_xml::de::from_reader(bs.reader()).expect("must success");

        assert_eq!(out.bucket, "Example-Bucket");
        assert_eq!(out.key, "Example-Object");
        assert_eq!(
            out.location,
            "http://Example-Bucket.s3.region.amazonaws.com/Example-Object"
        );
        assert_eq!(out.etag, "\"3858f62230ac3c915f300c664312c11f-9\"");
    }

    /// This example is from https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeleteObjects.html#API_DeleteObjects_Examples
    #[test]
    fn test_serialize_delete_objects_request() {
        let req = DeleteObjectsRequest {
            object: vec![
                DeleteObjectsRequestObject {
                    key: "sample1.txt".to_string(),
                    version_id: None,
                },
                DeleteObjectsRequestObject {
                    key: "sample2.txt".to_string(),
                    version_id: Some("11111".to_owned()),
                },
            ],
        };

        let actual = quick_xml::se::to_string(&req).expect("must succeed");

        pretty_assertions::assert_eq!(
            actual,
            r#"<Delete>
             <Object>
             <Key>sample1.txt</Key>
             </Object>
             <Object>
               <Key>sample2.txt</Key>
               <VersionId>11111</VersionId>
             </Object>
             </Delete>"#
                // Cleanup space and new line
                .replace([' ', '\n'], "")
        )
    }

    /// This example is from https://docs.aws.amazon.com/AmazonS3/latest/API/API_DeleteObjects.html#API_DeleteObjects_Examples
    #[test]
    fn test_deserialize_delete_objects_result() {
        let bs = Bytes::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <DeleteResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
             <Deleted>
               <Key>sample1.txt</Key>
             </Deleted>
             <Error>
              <Key>sample2.txt</Key>
              <Code>AccessDenied</Code>
              <Message>Access Denied</Message>
             </Error>
            </DeleteResult>"#,
        );

        let out: DeleteObjectsResult =
            quick_xml::de::from_reader(bs.reader()).expect("must success");

        assert_eq!(out.deleted.len(), 1);
        assert_eq!(out.deleted[0].key, "sample1.txt");
        assert_eq!(out.error.len(), 1);
        assert_eq!(out.error[0].key, "sample2.txt");
        assert_eq!(out.error[0].code, "AccessDenied");
        assert_eq!(out.error[0].message, "Access Denied");
    }

    #[test]
    fn test_parse_list_output() {
        let bs = bytes::Bytes::from(
            r#"<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>example-bucket</Name>
  <Prefix>photos/2006/</Prefix>
  <KeyCount>3</KeyCount>
  <MaxKeys>1000</MaxKeys>
  <Delimiter>/</Delimiter>
  <IsTruncated>false</IsTruncated>
  <Contents>
    <Key>photos/2006</Key>
    <LastModified>2016-04-30T23:51:29.000Z</LastModified>
    <ETag>"d41d8cd98f00b204e9800998ecf8427e"</ETag>
    <Size>56</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
  <Contents>
    <Key>photos/2007</Key>
    <LastModified>2016-04-30T23:51:29.000Z</LastModified>
    <ETag>"d41d8cd98f00b204e9800998ecf8427e"</ETag>
    <Size>100</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
  <Contents>
    <Key>photos/2008</Key>
    <LastModified>2016-05-30T23:51:29.000Z</LastModified>
    <Size>42</Size>
  </Contents>

  <CommonPrefixes>
    <Prefix>photos/2006/February/</Prefix>
  </CommonPrefixes>
  <CommonPrefixes>
    <Prefix>photos/2006/January/</Prefix>
  </CommonPrefixes>
</ListBucketResult>"#,
        );

        let out: ListObjectsOutput = quick_xml::de::from_reader(bs.reader()).expect("must success");

        assert!(!out.is_truncated.unwrap());
        assert!(out.next_continuation_token.is_none());
        assert_eq!(
            out.common_prefixes
                .iter()
                .map(|v| v.prefix.clone())
                .collect::<Vec<String>>(),
            vec!["photos/2006/February/", "photos/2006/January/"]
        );
        assert_eq!(
            out.contents,
            vec![
                ListObjectsOutputContent {
                    key: "photos/2006".to_string(),
                    size: 56,
                    etag: Some("\"d41d8cd98f00b204e9800998ecf8427e\"".to_string()),
                    last_modified: "2016-04-30T23:51:29.000Z".to_string(),
                },
                ListObjectsOutputContent {
                    key: "photos/2007".to_string(),
                    size: 100,
                    last_modified: "2016-04-30T23:51:29.000Z".to_string(),
                    etag: Some("\"d41d8cd98f00b204e9800998ecf8427e\"".to_string()),
                },
                ListObjectsOutputContent {
                    key: "photos/2008".to_string(),
                    size: 42,
                    last_modified: "2016-05-30T23:51:29.000Z".to_string(),
                    etag: None,
                },
            ]
        )
    }

    #[test]
    fn test_parse_list_object_versions() {
        let bs = bytes::Bytes::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
                <ListVersionsResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
                <Name>mtp-versioning-fresh</Name>
                <Prefix/>
                <KeyMarker>key3</KeyMarker>
                <VersionIdMarker>null</VersionIdMarker>
                <NextKeyMarker>key3</NextKeyMarker>
                <NextVersionIdMarker>d-d309mfjFrUmoQ0DBsVqmcMV15OI.</NextVersionIdMarker>
                <MaxKeys>3</MaxKeys>
                <IsTruncated>true</IsTruncated>
                <Version>
                    <Key>key3</Key>
                    <VersionId>8XECiENpj8pydEDJdd-_VRrvaGKAHOaGMNW7tg6UViI.</VersionId>
                    <IsLatest>true</IsLatest>
                    <LastModified>2009-12-09T00:18:23.000Z</LastModified>
                    <ETag>"396fefef536d5ce46c7537ecf978a360"</ETag>
                    <Size>217</Size>
                    <Owner>
                        <ID>75aa57f09aa0c8caeab4f8c24e99d10f8e7faeebf76c078efc7c6caea54ba06a</ID>
                    </Owner>
                    <StorageClass>STANDARD</StorageClass>
                </Version>
                <Version>
                    <Key>key3</Key>
                    <VersionId>d-d309mfjFri40QYukDozqBt3UmoQ0DBsVqmcMV15OI.</VersionId>
                    <IsLatest>false</IsLatest>
                    <LastModified>2009-12-09T00:18:08.000Z</LastModified>
                    <ETag>"396fefef536d5ce46c7537ecf978a360"</ETag>
                    <Size>217</Size>
                    <Owner>
                        <ID>75aa57f09aa0c8caeab4f8c24e99d10f8e7faeebf76c078efc7c6caea54ba06a</ID>
                    </Owner>
                    <StorageClass>STANDARD</StorageClass>
                </Version>
                <CommonPrefixes>
                    <Prefix>photos/</Prefix>
                </CommonPrefixes>
                <CommonPrefixes>
                    <Prefix>videos/</Prefix>
                </CommonPrefixes>
                 <DeleteMarker>
                    <Key>my-third-image.jpg</Key>
                    <VersionId>03jpff543dhffds434rfdsFDN943fdsFkdmqnh892</VersionId>
                    <IsLatest>true</IsLatest>
                    <LastModified>2009-10-15T17:50:30.000Z</LastModified>
                    <Owner>
                        <ID>75aa57f09aa0c8caeab4f8c24e99d10f8e7faeebf76c078efc7c6caea54ba06a</ID>
                        <DisplayName>mtd@amazon.com</DisplayName>
                    </Owner>
                </DeleteMarker>
                </ListVersionsResult>"#,
        );

        let output: ListObjectVersionsOutput =
            quick_xml::de::from_reader(bs.reader()).expect("must succeed");

        assert!(output.is_truncated.unwrap());
        assert_eq!(output.next_key_marker, Some("key3".to_owned()));
        assert_eq!(
            output.next_version_id_marker,
            Some("d-d309mfjFrUmoQ0DBsVqmcMV15OI.".to_owned())
        );
        assert_eq!(
            output.common_prefixes,
            vec![
                OutputCommonPrefix {
                    prefix: "photos/".to_owned()
                },
                OutputCommonPrefix {
                    prefix: "videos/".to_owned()
                }
            ]
        );

        assert_eq!(
            output.version,
            vec![
                ListObjectVersionsOutputVersion {
                    key: "key3".to_owned(),
                    version_id: "8XECiENpj8pydEDJdd-_VRrvaGKAHOaGMNW7tg6UViI.".to_owned(),
                    is_latest: true,
                    size: 217,
                    last_modified: "2009-12-09T00:18:23.000Z".to_owned(),
                    etag: Some("\"396fefef536d5ce46c7537ecf978a360\"".to_owned()),
                },
                ListObjectVersionsOutputVersion {
                    key: "key3".to_owned(),
                    version_id: "d-d309mfjFri40QYukDozqBt3UmoQ0DBsVqmcMV15OI.".to_owned(),
                    is_latest: false,
                    size: 217,
                    last_modified: "2009-12-09T00:18:08.000Z".to_owned(),
                    etag: Some("\"396fefef536d5ce46c7537ecf978a360\"".to_owned()),
                }
            ]
        );

        assert_eq!(
            output.delete_marker,
            vec![ListObjectVersionsOutputDeleteMarker {
                key: "my-third-image.jpg".to_owned(),
                version_id: "03jpff543dhffds434rfdsFDN943fdsFkdmqnh892".to_owned(),
                is_latest: true,
                last_modified: "2009-10-15T17:50:30.000Z".to_owned(),
            },]
        );
    }
}
