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

use bytes::Buf;
use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use http::request;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

use self::constants::*;
use opendal_core::raw::*;
use opendal_core::*;

pub(super) mod constants {
    // https://github.com/vercel/storage/blob/main/packages/blob/src/put.ts#L16
    // x-content-type specifies the MIME type of the file being uploaded.
    pub const X_VERCEL_BLOB_CONTENT_TYPE: &str = "x-content-type";
    // x-add-random-suffix specifying whether to  add a random suffix to the pathname
    // Default value is 1, which means to add a random suffix.
    // Set it to 0 to disable the random suffix.
    pub const X_VERCEL_BLOB_ADD_RANDOM_SUFFIX: &str = "x-add-random-suffix";
    // https://github.com/vercel/storage/blob/main/packages/blob/src/put-multipart.ts#L84
    // x-mpu-action specifies the action to perform on the MPU.
    // Possible values are:
    // - create: create a new MPU.
    // - upload: upload a part to an existing MPU.
    // - complete: complete an existing MPU.
    pub const X_VERCEL_BLOB_MPU_ACTION: &str = "x-mpu-action";
    pub const X_VERCEL_BLOB_MPU_KEY: &str = "x-mpu-key";
    pub const X_VERCEL_BLOB_MPU_PART_NUMBER: &str = "x-mpu-part-number";
    pub const X_VERCEL_BLOB_MPU_UPLOAD_ID: &str = "x-mpu-upload-id";
}

#[derive(Clone)]
pub struct VercelBlobCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    /// The root of this core.
    pub root: String,
    /// Vercel Blob token.
    pub token: String,
}

impl Debug for VercelBlobCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VercelBlobCore")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl VercelBlobCore {
    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    pub fn sign(&self, req: request::Builder) -> request::Builder {
        req.header(header::AUTHORIZATION, format!("Bearer {}", self.token))
    }
}

impl VercelBlobCore {
    pub async fn download(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        _: &OpRead,
    ) -> Result<Response<HttpBody>> {
        let p = build_abs_path(&self.root, path);
        // Vercel blob use an unguessable random id url to download the file
        // So we use list to get the url of the file and then use it to download the file
        let resp = self.list(ctx, &p, Some(1), None).await?;

        // Use the mtach url to download the file
        let url = resolve_blob(resp.blobs, p);

        if url.is_empty() {
            return Err(Error::new(ErrorKind::NotFound, "Blob not found"));
        }

        let mut req = Request::get(url);

        if !range.is_full() {
            req = req.header(header::RANGE, range.to_header());
        }

        // Set body
        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadBlob"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: u64,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "https://blob.vercel-storage.com/{}",
            percent_encode_path(&p)
        );

        let mut req = Request::put(&url);

        req = req.header(X_VERCEL_BLOB_ADD_RANDOM_SUFFIX, "0");

        req = req.header(header::CONTENT_LENGTH, size.to_string());

        if let Some(mime) = args.content_type() {
            req = req.header(X_VERCEL_BLOB_CONTENT_TYPE, mime)
        }

        let req = self.sign(req);

        // Set body
        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("PutBlob"))
            .body(body)
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn head(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let resp = self.list(ctx, &p, Some(1), None).await?;

        let url = resolve_blob(resp.blobs, p);

        if url.is_empty() {
            return Err(Error::new(ErrorKind::NotFound, "Blob not found"));
        }

        let req = Request::get(format!(
            "https://blob.vercel-storage.com?url={}",
            percent_encode_path(&url)
        ));

        let req = self.sign(req);

        // Set body
        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("HeadBlob"))
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
        let from = build_abs_path(&self.root, from);

        let resp = self.list(ctx, &from, Some(1), None).await?;

        let from_url = resolve_blob(resp.blobs, from);

        if from_url.is_empty() {
            return Err(Error::new(ErrorKind::NotFound, "Blob not found"));
        }

        let to = build_abs_path(&self.root, to);

        let to_url = format!(
            "https://blob.vercel-storage.com/{}?fromUrl={}",
            percent_encode_path(&to),
            percent_encode_path(&from_url),
        );

        let req = Request::put(&to_url);

        let req = self.sign(req);

        // Set body
        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyBlob"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn list(
        &self,
        ctx: &OperationContext,
        prefix: &str,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<ListResponse> {
        let prefix = if prefix == "/" { "" } else { prefix };

        let mut url = format!(
            "https://blob.vercel-storage.com?prefix={}",
            percent_encode_path(prefix)
        );

        if let Some(limit) = limit {
            url.push_str(&format!("&limit={limit}"))
        }

        if let Some(cursor) = cursor {
            url.push_str(&format!("&cursor={}", percent_encode_path(cursor)));
        }

        let req = Request::get(&url);

        let req = self.sign(req);

        // Set body
        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("ListBlobs"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        let status = resp.status();

        if status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListBlobs")),
                resp,
            ));
        }

        let body = resp.into_body();

        let resp: ListResponse =
            serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

        Ok(resp)
    }

    pub async fn vercel_delete_blob(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let p = build_abs_path(&self.root, path);

        let resp = self.list(ctx, &p, Some(1), None).await?;

        let url = resolve_blob(resp.blobs, p);

        if url.is_empty() {
            return Ok(());
        }

        let req = Request::post("https://blob.vercel-storage.com/delete");

        let req = self.sign(req);

        let req_body = &json!({
            "urls": vec![url]
        });

        // Set body
        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteBlob"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Buffer::from(Bytes::from(req_body.to_string())))
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("DeleteBlob")),
                resp,
            )),
        }
    }

    pub async fn initiate_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "https://blob.vercel-storage.com/mpu/{}",
            percent_encode_path(&p)
        );

        let req = Request::post(&url);

        let mut req = self.sign(req);

        req = req.header(X_VERCEL_BLOB_MPU_ACTION, "create");
        req = req.header(X_VERCEL_BLOB_ADD_RANDOM_SUFFIX, "0");

        if let Some(mime) = args.content_type() {
            req = req.header(X_VERCEL_BLOB_CONTENT_TYPE, mime);
        };

        // Set body
        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CreateMultipartUpload"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn upload_part(
        &self,
        ctx: &OperationContext,
        path: &str,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "https://blob.vercel-storage.com/mpu/{}",
            percent_encode_path(&p)
        );

        let mut req = Request::post(&url);

        req = req.header(header::CONTENT_LENGTH, size);
        req = req.header(X_VERCEL_BLOB_MPU_ACTION, "upload");
        req = req.header(X_VERCEL_BLOB_MPU_KEY, p);
        req = req.header(X_VERCEL_BLOB_MPU_UPLOAD_ID, upload_id);
        req = req.header(X_VERCEL_BLOB_MPU_PART_NUMBER, part_number);

        let req = self.sign(req);

        // Set body
        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadPart"))
            .body(body)
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn complete_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        upload_id: &str,
        parts: Vec<Part>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "https://blob.vercel-storage.com/mpu/{}",
            percent_encode_path(&p)
        );

        let mut req = Request::post(&url);

        req = req.header(X_VERCEL_BLOB_MPU_ACTION, "complete");
        req = req.header(X_VERCEL_BLOB_MPU_KEY, p);
        req = req.header(X_VERCEL_BLOB_MPU_UPLOAD_ID, upload_id);

        let req = self.sign(req);

        let parts_json = json!(parts);

        let req = req
            .header(header::CONTENT_TYPE, "application/json")
            .extension(Operation::Write)
            .extension(ServiceOperation("CompleteMultipartUpload"))
            .body(Buffer::from(Bytes::from(parts_json.to_string())))
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }
}

pub fn parse_blob(blob: &Blob) -> Result<Metadata> {
    let mut md = if blob.pathname.ends_with('/') {
        MetadataBuilder::dir()
    } else {
        MetadataBuilder::file(blob.size)
    };
    if let Some(content_type) = blob.content_type.clone() {
        md.content_type(&content_type);
    }
    md.last_modified(blob.uploaded_at.parse::<Timestamp>()?);
    md.content_disposition(&blob.content_disposition);
    Ok(md.build())
}

fn resolve_blob(blobs: Vec<Blob>, path: String) -> String {
    for blob in blobs {
        if blob.pathname == path {
            return blob.url;
        }
    }
    "".to_string()
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse {
    pub cursor: Option<String>,
    pub has_more: bool,
    pub blobs: Vec<Blob>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blob {
    pub url: String,
    pub pathname: String,
    pub size: u64,
    pub uploaded_at: String,
    pub content_disposition: String,
    pub content_type: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub part_number: usize,
    pub etag: String,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitiateMultipartUploadResponse {
    pub upload_id: String,
    pub key: String,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPartResponse {
    pub etag: String,
}

use quick_xml::de;

/// VercelBlobError is the error returned by VercelBlob service.
#[derive(Default, Debug, Deserialize)]
#[serde(default)]
struct VercelBlobError {
    error: VercelBlobErrorDetail,
}

#[derive(Default, Debug, Deserialize)]
#[serde(default)]
struct VercelBlobErrorDetail {
    code: String,
    message: Option<String>,
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
        500 | 502 | 503 | 504 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, _vercel_blob_err) = de::from_reader::<_, VercelBlobError>(bs.clone().reader())
        .map(|vercel_blob_err| (format!("{vercel_blob_err:?}"), Some(vercel_blob_err)))
        .unwrap_or_else(|_| (String::from_utf8_lossy(&bs).into_owned(), None));

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
        let err_res = vec![(
            r#"{
                    "error": {
                        "code": "forbidden",
                        "message": "Invalid token"
                    }
                }"#,
            ErrorKind::PermissionDenied,
            StatusCode::FORBIDDEN,
        )];

        for res in err_res {
            let body = Buffer::from(res.0.as_bytes().to_vec());
            let resp = Response::builder().status(res.2).body(body).unwrap();

            let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);

            assert_eq!(err.kind(), res.1);
        }

        let bs = bytes::Bytes::from(
            r#"{
                "error": {
                    "code": "forbidden",
                    "message": "Invalid token"
                }
            }"#,
        );

        let out: VercelBlobError = serde_json::from_reader(bs.reader()).expect("must success");
        println!("{out:?}");

        assert_eq!(out.error.code, "forbidden");
        assert_eq!(out.error.message, Some("Invalid token".to_string()));
    }
}
