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
use std::fmt::Write;

use backon::ExponentialBuilder;
use backon::Retryable;
use bytes::BytesMut;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::Request;
use http::Response;
use once_cell::sync::Lazy;
use reqsign::GoogleCredentialLoader;
use reqsign::GoogleSigner;
use reqsign::GoogleToken;
use reqsign::GoogleTokenLoader;

use super::uri::percent_encode_path;
use crate::raw::*;
use crate::*;

pub struct GcsCore {
    pub endpoint: String,
    pub bucket: String,
    pub root: String,

    pub client: HttpClient,
    pub signer: GoogleSigner,
    pub token_loader: GoogleTokenLoader,
    pub credential_loader: GoogleCredentialLoader,

    pub predefined_acl: Option<String>,
    pub default_storage_class: String,
}

impl Debug for GcsCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut de = f.debug_struct("Backend");
        de.field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("root", &self.root);
        if self.predefined_acl.is_some() {
            de.field("predefined_acl", &self.predefined_acl);
        }
        de.field("default_storage_class", &self.default_storage_class);
        de.finish_non_exhaustive()
    }
}

static BACKOFF: Lazy<ExponentialBuilder> =
    Lazy::new(|| ExponentialBuilder::default().with_jitter());

impl GcsCore {
    async fn load_token(&self) -> Result<GoogleToken> {
        let cred = { || self.token_loader.load() }
            .retry(&*BACKOFF)
            .await
            .map_err(new_request_credential_error)?;

        if let Some(cred) = cred {
            Ok(cred)
        } else {
            Err(Error::new(
                ErrorKind::ConfigInvalid,
                "no valid credential found",
            ))
        }
    }

    pub async fn sign<T>(&self, req: &mut Request<T>) -> Result<()> {
        let cred = self.load_token().await?;

        self.signer.sign(req, &cred).map_err(new_request_sign_error)
    }

    #[inline]
    pub async fn send(&self, req: Request<AsyncBody>) -> Result<Response<IncomingAsyncBody>> {
        self.client.send(req).await
    }
}

impl GcsCore {
    pub fn gcs_get_object_request(
        &self,
        path: &str,
        range: BytesRange,
    ) -> Result<Request<AsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/storage/v1/b/{}/o/{}?alt=media",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );

        let mut req = Request::get(&url);

        if !range.is_full() {
            req = req.header(http::header::RANGE, range.to_header());
        }

        let req = req
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn gcs_get_object(
        &self,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<IncomingAsyncBody>> {
        let mut req = self.gcs_get_object_request(path, range)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub fn gcs_insert_object_request(
        &self,
        path: &str,
        size: Option<usize>,
        content_type: Option<&str>,
        body: AsyncBody,
    ) -> Result<Request<AsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/upload/storage/v1/b/{}/o?uploadType=multipart&name={}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );

        if let Some(acl) = &self.predefined_acl {
            write!(&mut url, "&predefinedAcl={}", acl).unwrap();
        }

        let mut req =
            Request::post(&url).header(CONTENT_TYPE, "multipart/related; boundary=my-boundary");

        if let Some(size) = size {
            req = req.header(CONTENT_LENGTH, size)
        }

        let mut req_body = BytesMut::with_capacity(100);
        write!(
            &mut req_body,
            "--my-boundary\nContent-Type: application/json; charset=UTF-8\n\n{{\"storageClass\": \"{}\"}}\n\n--my-boundary\n",
            self.default_storage_class
        ).unwrap();

        if let Some(mime) = content_type {
            write!(&mut req_body, "Content-Type: {}\n\n", mime).unwrap();
        } else {
            write!(&mut req_body, "Content-Type: application/octet-stream\n\n").unwrap();
        }

        if let AsyncBody::Bytes(bytes) = body {
            req_body.extend_from_slice(&bytes);
        }
        write!(&mut req_body, "\n--my-boundary").unwrap();
        let req_body = AsyncBody::Bytes(req_body.freeze());
        let req = req.body(req_body).map_err(new_request_build_error)?;
        Ok(req)
    }

    pub async fn gcs_get_object_metadata(&self, path: &str) -> Result<Response<IncomingAsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/storage/v1/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );

        let req = Request::get(&url);

        let mut req = req
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn gcs_delete_object(&self, path: &str) -> Result<Response<IncomingAsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/storage/v1/b/{}/o/{}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );

        let mut req = Request::delete(&url)
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }

    pub async fn gcs_copy_object(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Response<IncomingAsyncBody>> {
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
            .body(AsyncBody::Empty)
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
    ) -> Result<Response<IncomingAsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/storage/v1/b/{}/o?prefix={}",
            self.endpoint,
            self.bucket,
            percent_encode_path(&p)
        );
        if !delimiter.is_empty() {
            write!(url, "&delimiter={delimiter}").expect("write into string must succeed");
        }
        if let Some(limit) = limit {
            write!(url, "&maxResults={limit}").expect("write into string must succeed");
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
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;

        self.sign(&mut req).await?;

        self.send(req).await
    }
}
