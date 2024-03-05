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

use http::Request;
use http::Response;
use reqsign::OCIAPIKeySigner;
use reqsign::OCICredential;
use reqsign::OCILoader;
use serde::Deserialize;
use serde::Serialize;

use crate::raw::*;
use crate::*;

mod constants {
    pub const QUERY_LIST_PREFIXY: &str = "prefix";
}

pub struct OciOsCore {
    pub region: String,
    pub root: String,
    pub bucket: String,
    pub namespace: String,

    pub client: HttpClient,
    pub loader: OCILoader,
    pub signer: OCIAPIKeySigner,
}

impl Debug for OciOsCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backend")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

impl OciOsCore {
    async fn load_credential(&self) -> Result<Option<OCICredential>> {
        let cred = self
            .loader
            .load()
            .await
            .map_err(new_request_credential_error)?;

        if let Some(cred) = cred {
            Ok(Some(cred))
        } else {
            // Mark this error as temporary since it could be caused by Aliyun STS.
            Err(Error::new(
                ErrorKind::PermissionDenied,
                "no valid credential found, please check configuration or try again",
            )
            .set_temporary())
        }
    }

    pub async fn sign<T>(&self, req: &mut Request<T>) -> Result<()> {
        let cred = if let Some(cred) = self.load_credential().await? {
            cred
        } else {
            return Ok(());
        };

        self.signer.sign(req, &cred).map_err(new_request_sign_error)
    }

    #[inline]
    pub async fn send(&self, req: Request<AsyncBody>) -> Result<Response<IncomingAsyncBody>> {
        self.client.send(req).await
    }
}

impl OciOsCore {
    pub fn os_get_object_request(
        &self,
        path: &str,
        _range: BytesRange,
        _is_presign: bool,
        _if_match: Option<&str>,
        _if_none_match: Option<&str>,
        _override_content_disposition: Option<&str>,
    ) -> Result<Request<AsyncBody>> {
        let p = build_abs_path(&self.root, path);
        let url = format!(
            "objectstorage.{}.oraclecloud.com/n/{}/b/{}/o/{}",
            self.region,
            self.namespace,
            self.bucket,
            percent_encode_path(&p)
        );

        let req = Request::get(&url)
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub fn os_head_object_request(
        &self,
        path: &str,
        _is_presign: bool,
        _if_match: Option<&str>,
        _if_none_match: Option<&str>,
    ) -> Result<Request<AsyncBody>> {
        let p = build_abs_path(&self.root, path);
        let url = format!(
            "objectstorage.{}.oraclecloud.com/n/{}/b/{}/o/{}",
            self.region,
            self.namespace,
            self.bucket,
            percent_encode_path(&p)
        );

        let req = Request::head(&url)
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub fn os_list_object_request(
        &self,
        path: &str,
        _delimiter: &str,
        _limit: Option<usize>,
        _start_after: Option<String>,
    ) -> Result<Request<AsyncBody>> {
        let p = build_abs_path(&self.root, path);
        let url = format!(
            "objectstorage.{}.oraclecloud.com/n/{}/b/{}/o",
            self.region, self.namespace, self.bucket,
        );

        // Add query arguments to the URL based on response overrides
        let mut query_args = Vec::new();
        query_args.push(format!("{}={}", constants::QUERY_LIST_PREFIXY, p,));

        let req = Request::get(&url)
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn os_list_object(
        &self,
        path: &str,
        _delimiter: &str,
        _limit: Option<usize>,
        start_after: Option<String>,
    ) -> Result<Response<IncomingAsyncBody>> {
        let mut req = self.os_list_object_request(path, _delimiter, _limit, start_after)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub async fn os_get_object(
        &self,
        path: &str,
        range: BytesRange,
        if_match: Option<&str>,
        if_none_match: Option<&str>,
        override_content_disposition: Option<&str>,
    ) -> Result<Response<IncomingAsyncBody>> {
        let mut req = self.os_get_object_request(
            path,
            range,
            false,
            if_match,
            if_none_match,
            override_content_disposition,
        )?;
        self.sign(&mut req).await?;
        self.send(req).await
    }

    pub async fn os_head_object(
        &self,
        path: &str,
        if_match: Option<&str>,
        if_none_match: Option<&str>,
    ) -> Result<Response<IncomingAsyncBody>> {
        let mut req = self.os_head_object_request(path, false, if_match, if_none_match)?;

        self.sign(&mut req).await?;
        self.send(req).await
    }
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
}

/// Result of DeleteObjects.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename = "DeleteResult", rename_all = "PascalCase")]
pub struct DeleteObjectsResult {
    pub deleted: Vec<DeleteObjectsResultDeleted>,
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

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct InitiateMultipartUploadResult {
    #[cfg(test)]
    pub bucket: String,
    #[cfg(test)]
    pub key: String,
    pub upload_id: String,
}

#[derive(Clone, Default, Debug, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct MultipartUploadPart {
    #[serde(rename = "PartNumber")]
    pub part_number: usize,
    #[serde(rename = "ETag")]
    pub etag: String,
}

#[derive(Default, Debug, Serialize)]
#[serde(default, rename = "CompleteMultipartUpload", rename_all = "PascalCase")]
pub struct CompleteMultipartUploadRequest {
    pub part: Vec<MultipartUploadPart>,
}

#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CompleteMultipartUploadResult {
    pub location: String,
    pub bucket: String,
    pub key: String,
    #[serde(rename = "ETag")]
    pub etag: String,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ListObjectsOutput {
    pub prefix: String,
    pub max_keys: u64,
    pub encoding_type: String,
    pub is_truncated: bool,
    pub common_prefixes: Vec<CommonPrefix>,
    pub contents: Vec<ListObjectsOutputContent>,
    pub key_count: u64,

    pub next_continuation_token: Option<String>,
}

#[derive(Default, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "PascalCase")]
pub struct ListObjectsOutputContent {
    pub key: String,
    pub last_modified: String,
    #[serde(rename = "ETag")]
    pub etag: String,
    pub size: u64,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct CommonPrefix {
    pub prefix: String,
}
