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
use http::header;
use http::request;
use serde::Deserialize;

use opendal_core::raw::*;
use opendal_core::*;

#[derive(Clone)]
pub struct YandexDiskCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    /// The root of this core.
    pub root: String,
    /// Yandex Disk oauth access_token.
    pub access_token: String,
}

impl Debug for YandexDiskCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("YandexDiskCore")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl YandexDiskCore {
    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    #[inline]
    pub fn sign(&self, req: request::Builder) -> request::Builder {
        req.header(
            header::AUTHORIZATION,
            format!("OAuth {}", self.access_token),
        )
    }
}

impl YandexDiskCore {
    /// Get upload url.
    async fn get_upload_url(&self, ctx: &OperationContext, path: &str) -> Result<String> {
        let path = build_rooted_abs_path(&self.root, path);

        let url = format!(
            "https://cloud-api.yandex.net/v1/disk/resources/upload?path={}&overwrite=true",
            percent_encode_path(&path)
        );

        let req = Request::get(url);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("GetUploadUrl"));

        let req = self.sign(req);

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bytes = resp.into_body();

                let resp: GetUploadUrlResponse =
                    serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;

                Ok(resp.href)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetUploadUrl")),
                resp,
            )),
        }
    }

    pub async fn upload(
        &self,
        ctx: &OperationContext,
        path: &str,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let upload_url = self.get_upload_url(ctx, path).await?;
        let req = Request::put(upload_url)
            .extension(Operation::Write)
            .extension(ServiceOperation("Upload"))
            .body(body)
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    async fn get_download_url(&self, ctx: &OperationContext, path: &str) -> Result<String> {
        let path = build_rooted_abs_path(&self.root, path);

        let url = format!(
            "https://cloud-api.yandex.net/v1/disk/resources/download?path={}&overwrite=true",
            percent_encode_path(&path)
        );

        let req = Request::get(url);

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetDownloadUrl"));

        let req = self.sign(req);

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bytes = resp.into_body();

                let resp: GetUploadUrlResponse =
                    serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;

                Ok(resp.href)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetDownloadUrl")),
                resp,
            )),
        }
    }

    pub async fn download(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let download_url = self.get_download_url(ctx, path).await?;
        let req = Request::get(download_url)
            .header(header::RANGE, range.to_header())
            .extension(Operation::Read)
            .extension(ServiceOperation("Download"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn ensure_dir_exists(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let path = build_abs_path(&self.root, path);

        let paths = path.split('/').collect::<Vec<&str>>();

        for i in 0..paths.len() - 1 {
            let path = paths[..i + 1].join("/");
            let resp = self.create_dir(ctx, &path).await?;

            let status = resp.status();

            match status {
                StatusCode::CREATED | StatusCode::CONFLICT => {}
                _ => {
                    return Err(parse_error(
                        ErrorContext::new(ServiceOperation("CreateResource")),
                        resp,
                    ));
                }
            }
        }
        Ok(())
    }

    pub async fn create_dir(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let url = format!(
            "https://cloud-api.yandex.net/v1/disk/resources?path=/{}",
            percent_encode_path(path),
        );

        let req = Request::put(url);

        let req = req
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("CreateResource"));

        let req = self.sign(req);

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_rooted_abs_path(&self.root, from);
        let to = build_rooted_abs_path(&self.root, to);

        let url = format!(
            "https://cloud-api.yandex.net/v1/disk/resources/copy?from={}&path={}&overwrite=true",
            percent_encode_path(&from),
            percent_encode_path(&to)
        );

        let req = Request::post(url);

        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyResource"));

        let req = self.sign(req);

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn move_object(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_rooted_abs_path(&self.root, from);
        let to = build_rooted_abs_path(&self.root, to);

        let url = format!(
            "https://cloud-api.yandex.net/v1/disk/resources/move?from={}&path={}&overwrite=true",
            percent_encode_path(&from),
            percent_encode_path(&to)
        );

        let req = Request::post(url);

        let req = req
            .extension(Operation::Rename)
            .extension(ServiceOperation("MoveResource"));

        let req = self.sign(req);

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn delete(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);

        let url = format!(
            "https://cloud-api.yandex.net/v1/disk/resources?path={}&permanently=true",
            percent_encode_path(&path),
        );

        let req = Request::delete(url);

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteResource"));

        let req = self.sign(req);

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn metainformation(
        &self,
        ctx: &OperationContext,
        path: &str,
        limit: Option<usize>,
        offset: Option<String>,
    ) -> Result<Response<Buffer>> {
        let path = build_rooted_abs_path(&self.root, path);

        let mut url = format!(
            "https://cloud-api.yandex.net/v1/disk/resources?path={}",
            percent_encode_path(&path),
        );

        if let Some(limit) = limit {
            url = format!("{url}&limit={limit}");
        }

        if let Some(offset) = offset {
            url = format!("{url}&offset={offset}");
        }

        let req = Request::get(url);

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetMetainformation"));

        let req = self.sign(req);

        // Set body
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }
}

pub(super) fn parse_info(mf: MetainformationResponse) -> Result<Metadata> {
    let mode = if mf.ty == "file" {
        EntryMode::FILE
    } else {
        EntryMode::DIR
    };

    let mut m = if mode == EntryMode::FILE {
        MetadataBuilder::file(mf.size.ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "yandex disk response does not contain file size",
            )
        })?)
    } else {
        MetadataBuilder::dir()
    };

    m.last_modified(mf.modified.parse::<Timestamp>()?);

    if let Some(md5) = mf.md5 {
        m.content_md5(&md5);
    }

    if let Some(mime_type) = mf.mime_type {
        m.content_type(&mime_type);
    }

    Ok(m.build())
}

#[derive(Debug, Deserialize)]
pub struct GetUploadUrlResponse {
    pub href: String,
}

#[derive(Debug, Deserialize)]
pub struct MetainformationResponse {
    #[serde(rename = "type")]
    pub ty: String,
    pub path: String,
    pub modified: String,
    pub md5: Option<String>,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
    #[serde(rename = "_embedded")]
    pub embedded: Option<Embedded>,
}

#[derive(Debug, Deserialize)]
pub struct Embedded {
    pub total: usize,
    pub items: Vec<MetainformationResponse>,
}

use quick_xml::de;

/// YandexDiskError is the error returned by YandexDisk service.
#[derive(Default, Debug, Deserialize)]
#[allow(unused)]
struct YandexDiskError {
    message: String,
    description: String,
    error: String,
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
        410 | 403 => (ErrorKind::PermissionDenied, false),
        404 => (ErrorKind::NotFound, false),
        // We should retry it when we get 423 error.
        423 => (ErrorKind::RateLimited, true),
        499 => (ErrorKind::Unexpected, true),
        503 | 507 => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let (message, _yandex_disk_err) = de::from_reader::<_, YandexDiskError>(bs.clone().reader())
        .map(|yandex_disk_err| (format!("{yandex_disk_err:?}"), Some(yandex_disk_err)))
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
                r#"{
                    "message": "Не удалось найти запрошенный ресурс.",
                    "description": "Resource not found.",
                    "error": "DiskNotFoundError"
                }"#,
                ErrorKind::NotFound,
                StatusCode::NOT_FOUND,
            ),
            (
                r#"{
                    "message": "Не авторизован.",
                    "description": "Unauthorized",
                    "error": "UnauthorizedError"
                }"#,
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
}
