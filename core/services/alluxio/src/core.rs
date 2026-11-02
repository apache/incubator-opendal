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
use http::Request;
use http::Response;
use http::StatusCode;
use serde::Deserialize;
use serde::Serialize;

use opendal_core::raw::*;
use opendal_core::*;

/// Alluxio core
#[derive(Clone)]
pub struct AlluxioCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    /// root of this backend.
    pub root: String,
    /// endpoint of alluxio
    pub endpoint: String,
}

impl Debug for AlluxioCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlluxioCore")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl AlluxioCore {
    pub async fn create_dir(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let path = build_rooted_abs_path(&self.root, path);

        let r = CreateDirRequest {
            recursive: Some(true),
            allow_exists: Some(true),
        };

        let body = serde_json::to_vec(&r).map_err(new_json_serialize_error)?;
        let body = bytes::Bytes::from(body);

        let mut req = Request::post(format!(
            "{}/api/v1/paths/{}/create-directory",
            self.endpoint,
            percent_encode_path(&path)
        ));

        req = req.header("Content-Type", "application/json");

        let req = req
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("CreateDirectory"));

        let req = req
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateDirectory")),
                resp,
            )),
        }
    }

    pub async fn create_file(&self, ctx: &OperationContext, path: &str) -> Result<u64> {
        let path = build_rooted_abs_path(&self.root, path);

        let r = CreateFileRequest {
            recursive: Some(true),
        };

        let body = serde_json::to_vec(&r).map_err(new_json_serialize_error)?;
        let body = bytes::Bytes::from(body);
        let mut req = Request::post(format!(
            "{}/api/v1/paths/{}/create-file",
            self.endpoint,
            percent_encode_path(&path)
        ));

        req = req.header("Content-Type", "application/json");

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CreateFile"));

        let req = req
            .body(Buffer::from(body))
            .map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;
        let status = resp.status();

        match status {
            StatusCode::OK => {
                let body = resp.into_body();
                let steam_id: u64 =
                    serde_json::from_reader(body.reader()).map_err(new_json_serialize_error)?;
                Ok(steam_id)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateFile")),
                resp,
            )),
        }
    }

    pub(super) async fn open_file(&self, ctx: &OperationContext, path: &str) -> Result<u64> {
        let path = build_rooted_abs_path(&self.root, path);

        let req = Request::post(format!(
            "{}/api/v1/paths/{}/open-file",
            self.endpoint,
            percent_encode_path(&path)
        ));

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("OpenFile"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let body = resp.into_body();
                let steam_id: u64 =
                    serde_json::from_reader(body.reader()).map_err(new_json_serialize_error)?;
                Ok(steam_id)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("OpenFile")),
                resp,
            )),
        }
    }

    pub(super) async fn delete(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let path = build_rooted_abs_path(&self.root, path);

        let req = Request::post(format!(
            "{}/api/v1/paths/{}/delete",
            self.endpoint,
            percent_encode_path(&path)
        ));

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("Delete"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(()),
            _ => {
                let err = parse_error(ErrorContext::new(ServiceOperation("Delete")), resp);
                if err.kind() == ErrorKind::NotFound {
                    return Ok(());
                }
                Err(err)
            }
        }
    }

    pub(super) async fn rename(&self, ctx: &OperationContext, path: &str, dst: &str) -> Result<()> {
        let path = build_rooted_abs_path(&self.root, path);
        let dst = build_rooted_abs_path(&self.root, dst);

        let req = Request::post(format!(
            "{}/api/v1/paths/{}/rename?dst={}",
            self.endpoint,
            percent_encode_path(&path),
            percent_encode_path(&dst)
        ));

        let req = req
            .extension(Operation::Rename)
            .extension(ServiceOperation("Rename"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Rename")),
                resp,
            )),
        }
    }

    pub(super) async fn get_status(&self, ctx: &OperationContext, path: &str) -> Result<FileInfo> {
        let path = build_rooted_abs_path(&self.root, path);

        let req = Request::post(format!(
            "{}/api/v1/paths/{}/get-status",
            self.endpoint,
            percent_encode_path(&path)
        ));

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetStatus"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let body = resp.into_body();
                let file_info: FileInfo =
                    serde_json::from_reader(body.reader()).map_err(new_json_serialize_error)?;
                Ok(file_info)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetStatus")),
                resp,
            )),
        }
    }

    pub(super) async fn list_status(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Vec<FileInfo>> {
        let path = build_rooted_abs_path(&self.root, path);

        let req = Request::post(format!(
            "{}/api/v1/paths/{}/list-status",
            self.endpoint,
            percent_encode_path(&path)
        ));

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("ListStatus"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let body = resp.into_body();
                let file_infos: Vec<FileInfo> =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;
                Ok(file_infos)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("ListStatus")),
                resp,
            )),
        }
    }

    pub async fn read(
        &self,
        ctx: &OperationContext,
        stream_id: u64,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        if !range.is_full() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "alluxio stream read doesn't support range",
            )
            .with_context("range", format!("{range:?}")));
        }

        let req = Request::post(format!(
            "{}/api/v1/streams/{}/read",
            self.endpoint, stream_id,
        ));

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("ReadStream"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub(super) async fn write(
        &self,
        ctx: &OperationContext,
        stream_id: u64,
        body: Buffer,
    ) -> Result<usize> {
        let req = Request::post(format!(
            "{}/api/v1/streams/{}/write",
            self.endpoint, stream_id
        ));

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("WriteStream"));

        let req = req.body(body).map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let body = resp.into_body();
                let size: usize =
                    serde_json::from_reader(body.reader()).map_err(new_json_serialize_error)?;
                Ok(size)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("WriteStream")),
                resp,
            )),
        }
    }

    pub(super) async fn close(&self, ctx: &OperationContext, stream_id: u64) -> Result<()> {
        let req = Request::post(format!(
            "{}/api/v1/streams/{}/close",
            self.endpoint, stream_id
        ));

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CloseStream"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CloseStream")),
                resp,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use http::StatusCode;

    use super::*;

    #[tokio::test]
    async fn test_read_rejects_range() {
        let core = AlluxioCore {
            info: ServiceInfo::new("alluxio", "", ""),
            capability: Capability::default(),
            root: "/".to_string(),
            endpoint: "http://127.0.0.1:1".to_string(),
        };

        let ctx = OperationContext::new();
        let err = match core.read(&ctx, 1, BytesRange::from(0_u64..1)).await {
            Ok(_) => panic!("range read should be rejected"),
            Err(err) => err,
        };

        assert_eq!(err.kind(), ErrorKind::Unsupported);
    }

    #[test]
    fn test_parse_error_marks_5xx_temporary() {
        for status in [500u16, 502, 503, 504] {
            let resp = Response::builder()
                .status(StatusCode::from_u16(status).unwrap())
                .body(Buffer::from(bytes::Bytes::from_static(
                    br#"{"statusCode":"INTERNAL_SERVER_ERROR","message":"boom"}"#,
                )))
                .unwrap();

            let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);

            assert!(err.is_temporary(), "{status} should be retryable");
        }

        let resp = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Buffer::from(bytes::Bytes::from_static(
                br#"{"statusCode":"INVALID_ARGUMENT","message":"bad"}"#,
            )))
            .unwrap();
        let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);
        assert!(!err.is_temporary(), "400 must not be retryable");
    }

    /// Error response example is from https://docs.aws.amazon.com/AmazonS3/latest/API/ErrorResponses.html
    #[test]
    fn test_parse_error() {
        let err_res = vec![
            (
                r#"{"statusCode":"ALREADY_EXISTS","message":"The resource you requested already exist"}"#,
                ErrorKind::AlreadyExists,
            ),
            (
                r#"{"statusCode":"NOT_FOUND","message":"The resource you requested does not exist"}"#,
                ErrorKind::NotFound,
            ),
            (
                r#"{"statusCode":"INTERNAL_SERVER_ERROR","message":"Internal server error"}"#,
                ErrorKind::Unexpected,
            ),
        ];

        for res in err_res {
            let bs = bytes::Bytes::from(res.0);
            let body = Buffer::from(bs);
            let resp = Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(body)
                .unwrap();

            let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);

            assert_eq!(err.kind(), res.1);
        }
    }
}

#[derive(Debug, Serialize)]
struct CreateFileRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    recursive: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateDirRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    recursive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_exists: Option<bool>,
}

/// Metadata of alluxio object
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FileInfo {
    /// The path of the object
    pub path: String,
    /// The last modification time of the object
    pub last_modification_time_ms: i64,
    /// Whether the object is a folder
    pub folder: bool,
    /// The length of the object in bytes
    pub length: u64,
}

impl TryFrom<FileInfo> for Metadata {
    type Error = Error;

    fn try_from(file_info: FileInfo) -> Result<Metadata> {
        let mut metadata = if file_info.folder {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::file(file_info.length)
        };
        metadata.last_modified(Timestamp::from_millisecond(
            file_info.last_modification_time_ms,
        )?);
        Ok(metadata.build())
    }
}

/// the error response of alluxio
#[derive(Default, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct AlluxioError {
    status_code: String,
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

    let (mut kind, retryable) = match parts.status.as_u16() {
        // Alluxio reports a transient master or worker failure as a 5xx; retrying is the
        // documented recovery, so mark it temporary rather than surfacing it as permanent.
        500 | 502 | 503 | 504 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, alluxio_err) = serde_json::from_reader::<_, AlluxioError>(bs.clone().reader())
        .map(|alluxio_err| (format!("{alluxio_err:?}"), Some(alluxio_err)))
        .unwrap_or_else(|_| (String::from_utf8_lossy(&bs).into_owned(), None));

    if let Some(alluxio_err) = alluxio_err {
        kind = match alluxio_err.status_code.as_str() {
            "ALREADY_EXISTS" => ErrorKind::AlreadyExists,
            "NOT_FOUND" => ErrorKind::NotFound,
            _ => ErrorKind::Unexpected,
        }
    }

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}
