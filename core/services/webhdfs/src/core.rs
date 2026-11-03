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

use asyncband::once::OnceCell;
use bytes::Buf;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use serde::Deserialize;

use opendal_core::raw::*;
use opendal_core::*;

pub struct WebhdfsCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub root: String,
    pub endpoint: String,
    pub user_name: Option<String>,
    pub auth: Option<String>,
    pub root_checker: OnceCell<()>,

    pub atomic_write_dir: Option<String>,
    pub disable_list_batch: bool,
}

impl Debug for WebhdfsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhdfsCore")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl WebhdfsCore {
    pub async fn webhdfs_create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/webhdfs/v1/{}?op=MKDIRS&overwrite=true&noredirect=true",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += format!("&{auth}").as_str();
        }

        let req = Request::put(&url);

        let req = req
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("Mkdirs"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    /// create object
    pub async fn webhdfs_create_object(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: Option<u64>,
        args: &OpWrite,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/webhdfs/v1/{}?op=CREATE&overwrite=true&noredirect=true",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += format!("&{auth}").as_str();
        }

        let req = Request::put(&url);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("Create"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        if status != StatusCode::CREATED && status != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("Create")),
                resp,
            ));
        }

        let bs = resp.into_body();

        let resp: LocationResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        let mut req = Request::put(&resp.location);

        if let Some(size) = size {
            req = req.header(CONTENT_LENGTH, size);
        };

        if let Some(content_type) = args.content_type() {
            req = req.header(CONTENT_TYPE, content_type);
        };
        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("Create"))
            .body(body)
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn webhdfs_rename_object(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
    ) -> Result<Response<Buffer>> {
        let from = build_abs_path(&self.root, from);
        let to = build_rooted_abs_path(&self.root, to);

        let mut url = format!(
            "{}/webhdfs/v1/{}?op=RENAME&destination={}",
            self.endpoint,
            percent_encode_path(&from),
            percent_encode_path(&to)
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += &format!("&{auth}");
        }

        let req = Request::put(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    async fn webhdfs_init_append(&self, ctx: &OperationContext, path: &str) -> Result<String> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=APPEND&noredirect=true",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += &format!("&{auth}");
        }

        let req = Request::post(url)
            .extension(Operation::Write)
            .extension(ServiceOperation("Append"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        let resp = ctx.http_transport().send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();
                let resp: LocationResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                Ok(resp.location)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Append")),
                resp,
            )),
        }
    }

    pub async fn webhdfs_append(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let mut url = self.webhdfs_init_append(ctx, path).await?;
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += &format!("&{auth}");
        }

        let mut req = Request::post(&url);

        req = req.header(CONTENT_LENGTH, size.to_string());

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("Append"))
            .body(body)
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    /// CONCAT will concat sources to the path
    pub async fn webhdfs_concat(
        &self,
        ctx: &OperationContext,
        path: &str,
        sources: Vec<String>,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let sources = sources
            .iter()
            .map(|p| build_rooted_abs_path(&self.root, p))
            .collect::<Vec<String>>()
            .join(",");

        let mut url = format!(
            "{}/webhdfs/v1/{}?op=CONCAT&sources={}",
            self.endpoint,
            percent_encode_path(&p),
            percent_encode_path(&sources),
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += &format!("&{auth}");
        }

        let req = Request::post(url);

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("Concat"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn webhdfs_list_status(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=LISTSTATUS",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += format!("&{auth}").as_str();
        }

        let req = Request::get(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        ctx.http_transport().send(req).await
    }

    pub async fn webhdfs_list_status_batch(
        &self,
        ctx: &OperationContext,
        path: &str,
        start_after: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!(
            "{}/webhdfs/v1/{}?op=LISTSTATUS_BATCH",
            self.endpoint,
            percent_encode_path(&p),
        );
        if !start_after.is_empty() {
            url += format!("&startAfter={}", percent_encode_path(start_after)).as_str();
        }
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += format!("&{auth}").as_str();
        }

        let req = Request::get(&url)
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        ctx.http_transport().send(req).await
    }

    fn webhdfs_open_request(&self, path: &str, range: &BytesRange) -> Result<Request<Buffer>> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=OPEN",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += &format!("&{auth}");
        }

        if !range.is_full() {
            url += &format!("&offset={}", range.offset());
            if let Some(size) = range.size() {
                url += &format!("&length={size}")
            }
        }

        let req = Request::get(&url)
            .extension(Operation::Read)
            .extension(ServiceOperation("Open"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    pub async fn webhdfs_read_file(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let req = self.webhdfs_open_request(path, &range)?;
        ctx.http_transport().fetch(req).await
    }

    pub(super) async fn webhdfs_get_file_status(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=GETFILESTATUS",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += format!("&{auth}").as_str();
        }

        let req = Request::get(&url)
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetFileStatus"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn webhdfs_delete(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=DELETE&recursive=false",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(user) = &self.user_name {
            url += format!("&user.name={user}").as_str();
        }
        if let Some(auth) = &self.auth {
            url += format!("&{auth}").as_str();
        }

        let req = Request::delete(&url)
            .extension(Operation::Delete)
            .extension(ServiceOperation("Delete"))
            .body(Buffer::new())
            .map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct LocationResponse {
    pub location: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WebHdfsErrorWrapper {
    pub remote_exception: WebHdfsError,
}

/// WebHdfsError is the error message returned by WebHdfs service
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct WebHdfsError {
    exception: String,
    message: String,
    java_class_name: String,
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
    let (kind, retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        // passing invalid arguments will return BAD_REQUEST
        // should be un-retryable
        StatusCode::BAD_REQUEST => (ErrorKind::Unexpected, false),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let message = match serde_json::from_slice::<WebHdfsErrorWrapper>(&bs) {
        Ok(wh_error) => format!("{:?}", wh_error.remote_exception),
        Err(_) => String::from_utf8_lossy(&bs).into_owned(),
    };

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
    use bytes::Buf;
    use serde_json::from_reader;

    use super::*;

    /// Error response example from https://hadoop.apache.org/docs/stable/hadoop-project-dist/hadoop-hdfs/WebHDFS.html#Error%20Responses
    #[tokio::test]
    async fn test_parse_error() -> Result<()> {
        let ill_args = bytes::Bytes::from(
            r#"
{
  "RemoteException":
  {
    "exception"    : "IllegalArgumentException",
    "javaClassName": "java.lang.IllegalArgumentException",
    "message"      : "Invalid value for webhdfs parameter \"permission\": ..."
  }
}
    "#,
        );
        let body = Buffer::from(ill_args.clone());
        let resp = Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(body)
            .unwrap();

        let err = parse_error(ErrorContext::new(ServiceOperation("Test")), resp);
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(!err.is_temporary());

        let err_msg: WebHdfsError = from_reader::<_, WebHdfsErrorWrapper>(ill_args.reader())
            .expect("must success")
            .remote_exception;
        assert_eq!(err_msg.exception, "IllegalArgumentException");
        assert_eq!(
            err_msg.java_class_name,
            "java.lang.IllegalArgumentException"
        );
        assert_eq!(
            err_msg.message,
            "Invalid value for webhdfs parameter \"permission\": ..."
        );

        Ok(())
    }
}
