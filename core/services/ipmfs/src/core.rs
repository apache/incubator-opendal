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
use std::fmt::Write;

use http::Request;
use http::Response;

use opendal_core::raw::*;
use opendal_core::*;

pub struct IpmfsCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub root: String,
    pub endpoint: String,
}

impl Debug for IpmfsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpmfsCore")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl IpmfsCore {
    pub async fn ipmfs_stat(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path);

        let url = format!(
            "{}/api/v0/files/stat?arg={}",
            self.endpoint,
            percent_encode_path(&p)
        );

        let req = Request::post(url)
            .extension(Operation::Stat)
            .extension(ServiceOperation("Stat"));
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn ipmfs_read(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let p = build_rooted_abs_path(&self.root, path);

        let mut url = format!(
            "{}/api/v0/files/read?arg={}",
            self.endpoint,
            percent_encode_path(&p)
        );

        write!(url, "&offset={}", range.offset()).expect("write into string must succeed");
        if let Some(count) = range.size() {
            write!(url, "&count={count}").expect("write into string must succeed")
        }

        let req = Request::post(url)
            .extension(Operation::Read)
            .extension(ServiceOperation("Read"));
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().fetch(req).await
    }

    pub async fn ipmfs_rm(&self, ctx: &OperationContext, path: &str) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path);

        let url = format!(
            "{}/api/v0/files/rm?arg={}",
            self.endpoint,
            percent_encode_path(&p)
        );

        let req = Request::post(url)
            .extension(Operation::Delete)
            .extension(ServiceOperation("Rm"));
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub(crate) async fn ipmfs_ls(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path);

        let url = format!(
            "{}/api/v0/files/ls?arg={}&long=true",
            self.endpoint,
            percent_encode_path(&p)
        );

        let req = Request::post(url)
            .extension(Operation::List)
            .extension(ServiceOperation("Ls"));
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    pub async fn ipmfs_mkdir(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path);

        let url = format!(
            "{}/api/v0/files/mkdir?arg={}&parents=true",
            self.endpoint,
            percent_encode_path(&p)
        );

        let req = Request::post(url)
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("Mkdir"));
        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;

        ctx.http_transport().send(req).await
    }

    /// Support write from reader.
    pub async fn ipmfs_write(
        &self,
        ctx: &OperationContext,
        path: &str,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let p = build_rooted_abs_path(&self.root, path);

        let url = format!(
            "{}/api/v0/files/write?arg={}&parents=true&create=true&truncate=true",
            self.endpoint,
            percent_encode_path(&p)
        );

        let multipart = Multipart::new().part(FormDataPart::new("data").content(body));

        let req: http::request::Builder = Request::post(url)
            .extension(Operation::Write)
            .extension(ServiceOperation("Write"));
        let req = multipart.apply(req)?;

        ctx.http_transport().send(req).await
    }
}

use http::StatusCode;
use serde::Deserialize;
use serde_json::de;

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct IpfsError {
    #[serde(rename = "Message")]
    message: String,
    #[serde(rename = "Code")]
    code: usize,
    #[serde(rename = "Type")]
    ty: String,
}

/// Parse error response into io::Error.
///
/// > Status code 500 means that the function does exist, but IPFS was not
/// > able to fulfil the request because of an error.
/// > To know that reason, you have to look at the error message that is
/// > usually returned with the body of the response
/// > (if no error, check the daemon logs).
///
/// ref: https://docs.ipfs.tech/reference/kubo/rpc/#http-status-codes
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

    let ipfs_error = de::from_slice::<IpfsError>(&bs).ok();

    let (kind, retryable) = match parts.status {
        StatusCode::INTERNAL_SERVER_ERROR => {
            if let Some(ie) = &ipfs_error {
                match ie.message.as_str() {
                    "file does not exist" => (ErrorKind::NotFound, false),
                    _ => (ErrorKind::Unexpected, false),
                }
            } else {
                (ErrorKind::Unexpected, false)
            }
        }
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT => {
            (ErrorKind::Unexpected, true)
        }
        _ => (ErrorKind::Unexpected, false),
    };

    let message = match ipfs_error {
        Some(ipfs_error) => format!("{ipfs_error:?}"),
        None => String::from_utf8_lossy(&bs).into_owned(),
    };

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}
