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

use base64::Engine;
use hmac::Hmac;
use hmac::KeyInit;
use hmac::Mac;
use http::HeaderMap;
use http::Request;
use http::Response;
use http::header;
use md5::Digest;
use opendal_core::raw::*;
use opendal_core::*;
use serde::Deserialize;
use sha1::Sha1;

use self::constants::*;

pub(super) mod constants {
    pub const X_UPYUN_FILE_TYPE: &str = "x-upyun-file-type";
    pub const X_UPYUN_FILE_SIZE: &str = "x-upyun-file-size";
    pub const X_UPYUN_CACHE_CONTROL: &str = "x-upyun-meta-cache-control";
    pub const X_UPYUN_CONTENT_DISPOSITION: &str = "x-upyun-meta-content-disposition";
    pub const X_UPYUN_MULTI_STAGE: &str = "X-Upyun-Multi-Stage";
    pub const X_UPYUN_MULTI_TYPE: &str = "X-Upyun-Multi-Type";
    pub const X_UPYUN_MULTI_DISORDER: &str = "X-Upyun-Multi-Disorder";
    pub const X_UPYUN_MULTI_UUID: &str = "X-Upyun-Multi-Uuid";
    pub const X_UPYUN_PART_ID: &str = "X-Upyun-Part-Id";
    pub const X_UPYUN_FOLDER: &str = "x-upyun-folder";
    pub const X_UPYUN_MOVE_SOURCE: &str = "X-Upyun-Move-Source";
    pub const X_UPYUN_COPY_SOURCE: &str = "X-Upyun-Copy-Source";
    pub const X_UPYUN_METADATA_DIRECTIVE: &str = "X-Upyun-Metadata-Directive";
    pub const X_UPYUN_LIST_ITER: &str = "x-list-iter";
    pub const X_UPYUN_LIST_LIMIT: &str = "X-List-Limit";
    pub const X_UPYUN_LIST_MAX_LIMIT: usize = 4096;
    pub const X_UPYUN_LIST_DEFAULT_LIMIT: usize = 256;
}

#[derive(Clone)]
pub struct UpyunCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    /// The root of this core.
    pub root: String,
    /// The endpoint of this backend.
    pub operator: String,
    /// The bucket of this backend.
    pub bucket: String,

    /// signer of this backend.
    pub signer: UpyunSigner,
}

/// Build the folder key for a directory path, without its trailing slash.
///
/// `build_abs_path` returns an empty string for the root of a service whose root is `/`, so
/// slicing off the last byte underflows there. `Operator::create_dir` only checks that the path
/// ends with `/`, and `/` does, so the root reaches this code and a library call panics instead of
/// returning an error. `trim_end_matches` is what the sibling services use and is total.
fn folder_path(root: &str, path: &str) -> String {
    build_abs_path(root, path).trim_end_matches('/').to_string()
}

impl Debug for UpyunCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpyunCore")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("operator", &self.operator)
            .finish_non_exhaustive()
    }
}

impl UpyunCore {
    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    pub fn sign(&self, req: &mut Request<Buffer>) -> Result<()> {
        // get rfc1123 date
        let date = Timestamp::now().format_http_date();
        let authorization =
            self.signer
                .authorization(&date, req.method().as_str(), req.uri().path());

        req.headers_mut()
            .insert("Authorization", authorization.parse().unwrap());
        req.headers_mut().insert("Date", date.parse().unwrap());

        Ok(())
    }
}

impl UpyunCore {
    pub async fn download_file(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&path)
        );

        let req = Request::get(url);

        let mut req = req
            .header(header::RANGE, range.to_header())
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadFile"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn info(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&path)
        );

        let req = Request::head(url);

        let mut req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetFileInfo"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        self.send(ctx, req).await
    }

    pub fn upload(
        &self,
        path: &str,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&p)
        );

        let mut req = Request::put(&url);

        if let Some(size) = size {
            req = req.header(header::CONTENT_LENGTH, size.to_string())
        }

        if let Some(mime) = args.content_type() {
            req = req.header(header::CONTENT_TYPE, mime)
        }

        if let Some(pos) = args.content_disposition() {
            req = req.header(X_UPYUN_CONTENT_DISPOSITION, pos)
        }

        if let Some(cache_control) = args.cache_control() {
            req = req.header(X_UPYUN_CACHE_CONTROL, cache_control)
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadFile"));

        // Set body
        let mut req = req.body(body).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        Ok(req)
    }

    pub async fn delete(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&path)
        );

        let req = Request::delete(url);

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteFile"));

        let mut req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        self.send(ctx, req).await
    }

    pub async fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = format!("/{}/{}", self.bucket, build_abs_path(&self.root, from));
        let to = build_abs_path(&self.root, to);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&to)
        );

        let mut req = Request::put(url);

        req = req.header(header::CONTENT_LENGTH, "0");

        req = req.header(X_UPYUN_COPY_SOURCE, from);

        req = req.header(X_UPYUN_METADATA_DIRECTIVE, "copy");

        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyFile"));

        // Set body
        let mut req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        self.send(ctx, req).await
    }

    pub async fn move_object(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = format!("/{}/{}", self.bucket, build_abs_path(&self.root, from));
        let to = build_abs_path(&self.root, to);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&to)
        );

        let mut req = Request::put(url);

        req = req.header(header::CONTENT_LENGTH, "0");

        req = req.header(X_UPYUN_MOVE_SOURCE, from);

        req = req.header(X_UPYUN_METADATA_DIRECTIVE, "copy");

        let req = req
            .extension(Operation::Rename)
            .extension(ServiceOperation("MoveFile"));

        // Set body
        let mut req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        self.send(ctx, req).await
    }

    pub async fn create_dir(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = folder_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&path)
        );

        let mut req = Request::post(url);

        req = req.header("folder", "true");

        req = req.header(X_UPYUN_FOLDER, "true");

        let req = req
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("CreateFolder"));

        let mut req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        self.send(ctx, req).await
    }

    pub async fn initiate_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&path)
        );

        let mut req = Request::put(url);

        req = req.header(X_UPYUN_MULTI_STAGE, "initiate");

        req = req.header(X_UPYUN_MULTI_DISORDER, "true");

        if let Some(content_type) = args.content_type() {
            req = req.header(X_UPYUN_MULTI_TYPE, content_type);
        }

        if let Some(content_disposition) = args.content_disposition() {
            req = req.header(X_UPYUN_CONTENT_DISPOSITION, content_disposition)
        }

        if let Some(cache_control) = args.cache_control() {
            req = req.header(X_UPYUN_CACHE_CONTROL, cache_control)
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("InitiateMultipartUpload"));

        let mut req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        self.send(ctx, req).await
    }

    pub fn upload_part(
        &self,
        path: &str,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&p),
        );

        let mut req = Request::put(&url);

        req = req.header(header::CONTENT_LENGTH, size);

        req = req.header(X_UPYUN_MULTI_STAGE, "upload");

        req = req.header(X_UPYUN_MULTI_UUID, upload_id);

        req = req.header(X_UPYUN_PART_ID, part_number);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadPart"));

        // Set body
        let mut req = req.body(body).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        Ok(req)
    }

    pub async fn complete_multipart_upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        upload_id: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&p),
        );

        let mut req = Request::put(url);

        req = req.header(X_UPYUN_MULTI_STAGE, "complete");

        req = req.header(X_UPYUN_MULTI_UUID, upload_id);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CompleteMultipartUpload"));

        let mut req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        self.send(ctx, req).await
    }

    pub async fn list_objects(
        &self,
        ctx: &OperationContext,
        path: &str,
        iter: &str,
        limit: Option<usize>,
    ) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "https://v0.api.upyun.com/{}/{}",
            self.bucket,
            percent_encode_path(&path),
        );

        let mut req = Request::get(url.clone());

        req = req.header(header::ACCEPT, "application/json");

        if !iter.is_empty() {
            req = req.header(X_UPYUN_LIST_ITER, iter);
        }

        if let Some(mut limit) = limit {
            if limit > X_UPYUN_LIST_MAX_LIMIT {
                limit = X_UPYUN_LIST_DEFAULT_LIMIT;
            }
            req = req.header(X_UPYUN_LIST_LIMIT, limit);
        }

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("ListObjects"));

        // Set body
        let mut req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.sign(&mut req)?;

        self.send(ctx, req).await
    }
}

#[derive(Clone, Default)]
pub struct UpyunSigner {
    pub operator: String,
    pub password: String,
}

type HmacSha1 = Hmac<Sha1>;

impl UpyunSigner {
    pub fn authorization(&self, date: &str, method: &str, uri: &str) -> String {
        let sign = vec![method, uri, date];

        let sign = sign
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<&str>>()
            .join("&");

        let mut mac = HmacSha1::new_from_slice(format_md5(self.password.as_bytes()).as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(sign.as_bytes());
        let sign_str = mac.finalize().into_bytes();

        let sign = base64::engine::general_purpose::STANDARD.encode(sign_str);
        format!("UPYUN {}:{}", self.operator, sign)
    }
}

pub(super) fn parse_info(headers: &HeaderMap) -> Result<Metadata> {
    let mode = if parse_header_to_str(headers, X_UPYUN_FILE_TYPE)? == Some("file") {
        EntryMode::FILE
    } else {
        EntryMode::DIR
    };

    let size = parse_header_to_str(headers, X_UPYUN_FILE_SIZE)?
        .map(|value| {
            value.parse::<u64>().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "header value is not valid integer")
                    .with_operation("parse_info")
                    .set_source(e)
            })
        })
        .transpose()?;
    let mut m = if mode == EntryMode::FILE {
        MetadataBuilder::file(size.ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "upyun response does not contain file size",
            )
        })?)
    } else {
        MetadataBuilder::dir()
    };

    if let Some(v) = parse_content_type(headers)? {
        m.content_type(v);
    }

    if let Some(v) = parse_content_md5(headers)? {
        m.content_md5(v);
    }

    if let Some(v) = parse_header_to_str(headers, X_UPYUN_CACHE_CONTROL)? {
        m.cache_control(v);
    }

    if let Some(v) = parse_header_to_str(headers, X_UPYUN_CONTENT_DISPOSITION)? {
        m.content_disposition(v);
    }

    Ok(m.build())
}

pub fn format_md5(bs: &[u8]) -> String {
    let mut hasher = md5::Md5::new();
    hasher.update(bs);

    format_digest_hex(hasher.finalize())
}

fn format_digest_hex(digest: impl AsRef<[u8]>) -> String {
    use std::fmt::Write;

    let digest = digest.as_ref();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String must succeed");
    }
    output
}

#[derive(Debug, Deserialize)]
pub(super) struct File {
    #[serde(rename = "type")]
    pub type_field: String,
    pub name: String,
    pub length: u64,
    pub last_modified: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListObjectsResponse {
    pub iter: String,
    pub files: Vec<File>,
}

use bytes::Buf;
use quick_xml::de;

/// UpyunError is the error returned by upyun service.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct UpyunError {
    code: i64,
    msg: String,
    id: String,
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
        304 | 412 => (ErrorKind::ConditionNotMatch, false),
        // Service like Upyun could return 499 error with a message like:
        // Client Disconnect, we should retry it.
        499 => (ErrorKind::Unexpected, true),
        500 | 502 | 503 | 504 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, _upyun_err) = de::from_reader::<_, UpyunError>(bs.clone().reader())
        .map(|upyun_err| (format!("{upyun_err:?}"), Some(upyun_err)))
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
        let err_res = vec![
            (
                r#"{"code": 40100016, "msg": "invalid date value in header", "id": "f5b30c720ddcecc70abd2f5c1c64bde8"}"#,
                ErrorKind::Unexpected,
                StatusCode::UNAUTHORIZED,
            ),
            (
                r#"{"code": 40300010, "msg": "file type error", "id": "f5b30c720ddcecc70abd2f5c1c64bde7"}"#,
                ErrorKind::PermissionDenied,
                StatusCode::FORBIDDEN,
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

    #[test]
    fn folder_path_handles_the_service_root() {
        // build_abs_path yields "" here; the previous `path[..path.len() - 1]` underflowed.
        assert_eq!(folder_path("/", "/"), "");
    }

    #[test]
    fn folder_path_drops_the_trailing_slash() {
        assert_eq!(folder_path("/", "a/b/"), "a/b");
        assert_eq!(folder_path("/prefix/", "a/"), "prefix/a");
    }
}
