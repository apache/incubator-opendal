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
use opendal_core::raw::*;
use opendal_core::*;
use serde::Deserialize;

#[derive(Clone)]
pub struct PcloudCore {
    pub info: ServiceInfo,
    pub capability: Capability,

    /// The root of this core.
    pub root: String,
    /// The endpoint of this backend.
    pub endpoint: String,
    /// The username id of this backend.
    pub username: String,
    /// The password of this backend.
    pub password: String,
}

impl Debug for PcloudCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcloudCore")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

impl PcloudCore {
    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }
}

impl PcloudCore {
    pub async fn get_file_link(&self, ctx: &OperationContext, path: &str) -> Result<String> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "{}/getfilelink?path=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(&path),
            self.username,
            self.password
        );

        let req = Request::get(url);

        // set body
        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetFileLink"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let bs = resp.into_body();
                let resp: GetFileLinkResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                let result = resp.result;
                if result == 2010 || result == 2055 || result == 2002 {
                    return Err(Error::new(ErrorKind::NotFound, format!("{resp:?}")));
                }
                if result != 0 {
                    return Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")));
                }

                if let Some(hosts) = resp.hosts
                    && let Some(path) = resp.path
                    && !hosts.is_empty()
                {
                    return Ok(format!("https://{}{}", hosts[0], path));
                }
                Err(Error::new(ErrorKind::Unexpected, "hosts is empty"))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetFileLink")),
                resp,
            )),
        }
    }

    pub async fn download(
        &self,
        ctx: &OperationContext,
        url: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let req = Request::get(url);

        // set body
        let req = req
            .header(header::RANGE, range.to_header())
            .extension(Operation::Read)
            .extension(ServiceOperation("DownloadFile"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn ensure_dir_exists(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let path = build_abs_path(&self.root, path);

        let paths = path.split('/').collect::<Vec<&str>>();

        for i in 0..paths.len() - 1 {
            let path = paths[..i + 1].join("/");
            let resp = self.create_folder_if_not_exists(ctx, &path).await?;

            let status = resp.status();

            match status {
                StatusCode::OK => {
                    let bs = resp.into_body();
                    let resp: PcloudError =
                        serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                    let result = resp.result;
                    if result == 2010 || result == 2055 || result == 2002 {
                        return Err(Error::new(ErrorKind::NotFound, format!("{resp:?}")));
                    }
                    if result != 0 {
                        return Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")));
                    }

                    if result != 0 {
                        return Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")));
                    }
                }
                _ => {
                    return Err(parse_error(
                        ErrorContext::new(ServiceOperation("CreateFolderIfNotExists")),
                        resp,
                    ));
                }
            }
        }
        Ok(())
    }

    pub async fn create_folder_if_not_exists(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let url = format!(
            "{}/createfolderifnotexists?path=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(path),
            self.username,
            self.password
        );

        let req = Request::post(url);

        // set body
        let req = req
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("CreateFolderIfNotExists"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn rename_file(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_abs_path(&self.root, from);
        let to = build_abs_path(&self.root, to);

        let url = format!(
            "{}/renamefile?path=/{}&topath=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(&from),
            percent_encode_path(&to),
            self.username,
            self.password
        );

        let req = Request::post(url);

        // set body
        let req = req
            .extension(Operation::Rename)
            .extension(ServiceOperation("RenameFile"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn rename_folder(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_abs_path(&self.root, from);
        let to = build_abs_path(&self.root, to);
        let url = format!(
            "{}/renamefolder?path=/{}&topath=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(&from),
            percent_encode_path(&to),
            self.username,
            self.password
        );

        let req = Request::post(url);

        // set body
        let req = req
            .extension(Operation::Rename)
            .extension(ServiceOperation("RenameFolder"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn delete_folder(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "{}/deletefolder?path=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(&path),
            self.username,
            self.password
        );

        let req = Request::post(url);

        // set body
        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteFolder"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn delete_file(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let url = format!(
            "{}/deletefile?path=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(&path),
            self.username,
            self.password
        );

        let req = Request::post(url);

        // set body
        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteFile"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn copy_file(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_abs_path(&self.root, from);
        let to = build_abs_path(&self.root, to);

        let url = format!(
            "{}/copyfile?path=/{}&topath=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(&from),
            percent_encode_path(&to),
            self.username,
            self.password
        );

        let req = Request::post(url);

        // set body
        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyFile"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn copy_folder(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_abs_path(&self.root, from);
        let to = build_abs_path(&self.root, to);

        let url = format!(
            "{}/copyfolder?path=/{}&topath=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(&from),
            percent_encode_path(&to),
            self.username,
            self.password
        );

        let req = Request::post(url);

        // set body
        let req = req
            .extension(Operation::Copy)
            .extension(ServiceOperation("CopyFolder"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn stat(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let path = path.trim_end_matches('/');

        let url = format!(
            "{}/stat?path=/{}&username={}&password={}",
            self.endpoint,
            percent_encode_path(path),
            self.username,
            self.password
        );

        let req = Request::post(url);

        // set body
        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("Stat"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn upload_file(
        &self,
        ctx: &OperationContext,
        path: &str,
        bs: Buffer,
    ) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let (name, path) = (get_basename(&path), get_parent(&path).trim_end_matches('/'));

        let url = format!(
            "{}/uploadfile?path=/{}&filename={}&username={}&password={}",
            self.endpoint,
            percent_encode_path(path),
            percent_encode_path(name),
            self.username,
            self.password
        );

        let req = Request::put(url);

        // set body
        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("UploadFile"))
            .body(bs)
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }

    pub async fn list_folder(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let path = build_abs_path(&self.root, path);

        let path = normalize_root(&path);

        let path = path.trim_end_matches('/');

        let path = if path.is_empty() { "/" } else { path };

        let url = format!(
            "{}/listfolder?path={}&username={}&password={}",
            self.endpoint,
            percent_encode_path(path),
            self.username,
            self.password
        );

        let req = Request::get(url);

        // set body
        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("ListFolder"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        self.send(ctx, req).await
    }
}

pub(super) fn parse_stat_metadata(content: StatMetadata) -> Result<Metadata> {
    let mut md = if content.isfolder {
        MetadataBuilder::dir()
    } else {
        MetadataBuilder::file(content.size.ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "pcloud stat response does not contain file size",
            )
        })?)
    };

    md.last_modified(Timestamp::parse_rfc2822(&content.modified)?);

    Ok(md.build())
}

pub(super) fn parse_list_metadata(content: ListMetadata) -> Result<Metadata> {
    let mut md = if content.isfolder {
        MetadataBuilder::dir()
    } else {
        MetadataBuilder::file(content.size.ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "pcloud list response does not contain file size",
            )
        })?)
    };

    md.last_modified(Timestamp::parse_rfc2822(&content.modified)?);

    Ok(md.build())
}

#[derive(Debug, Deserialize)]
pub struct GetFileLinkResponse {
    pub result: u64,
    pub path: Option<String>,
    pub hosts: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct StatResponse {
    pub result: u64,
    pub metadata: Option<StatMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct StatMetadata {
    pub modified: String,
    pub isfolder: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ListFolderResponse {
    pub result: u64,
    pub metadata: Option<ListMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct ListMetadata {
    pub path: String,
    pub modified: String,
    pub isfolder: bool,
    pub size: Option<u64>,
    pub contents: Option<Vec<ListMetadata>>,
}

/// PcloudError is the error returned by Pcloud service.
#[derive(Default, Deserialize)]
pub(crate) struct PcloudError {
    pub result: u32,
    pub error: Option<String>,
}

impl Debug for PcloudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcloudError")
            .field("result", &self.result)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
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
    let message = String::from_utf8_lossy(&bs).into_owned();

    // pCloud returns a 5xx for a transient backend failure; retrying is the documented
    // recovery, so mark it temporary rather than surfacing it as permanent.
    let retryable = matches!(parts.status.as_u16(), 500 | 502 | 503 | 504);

    let mut err = Error::new(ErrorKind::Unexpected, message);

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
            r#"<html>

                <head>
                    <title>Invalid link</title>
                </head>

                <body>This link was generated for another IP address. Try previous step again.</body>

                </html> "#,
            ErrorKind::Unexpected,
            StatusCode::GONE,
        )];

        for res in err_res {
            let bs = bytes::Bytes::from(res.0);
            let body = Buffer::from(bs);
            let resp = Response::builder().status(res.2).body(body).unwrap();

            let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);

            assert_eq!(err.kind(), res.1);
        }
    }
}
