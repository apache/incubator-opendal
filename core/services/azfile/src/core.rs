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

use std::collections::VecDeque;
use std::fmt::Debug;

use http::HeaderName;
use http::HeaderValue;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header::CONTENT_DISPOSITION;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::header::RANGE;
use reqsign_azure_storage::Credential;
use reqsign_core::{Context, Signer};

use opendal_core::raw::*;
use opendal_core::*;

const X_MS_VERSION: &str = "x-ms-version";
const X_MS_WRITE: &str = "x-ms-write";
const X_MS_FILE_RENAME_SOURCE: &str = "x-ms-file-rename-source";
const X_MS_CONTENT_LENGTH: &str = "x-ms-content-length";
const X_MS_TYPE: &str = "x-ms-type";
const X_MS_FILE_RENAME_REPLACE_IF_EXISTS: &str = "x-ms-file-rename-replace-if-exists";
pub const X_MS_META_PREFIX: &str = "x-ms-meta-";

pub struct AzfileCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub root: String,
    pub endpoint: String,
    pub share_name: String,
    pub signer: Signer<Credential>,
    pub sign_ctx: Context,
}

impl Debug for AzfileCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzfileCore")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("share_name", &self.share_name)
            .finish_non_exhaustive()
    }
}

impl AzfileCore {
    fn signer(&self, ctx: &OperationContext) -> Signer<Credential> {
        self.signer.clone().with_context(
            self.sign_ctx
                .clone()
                .with_http_send(ctx.http_transport().clone()),
        )
    }

    pub async fn sign<T>(&self, ctx: &OperationContext, req: Request<T>) -> Result<Request<T>> {
        let (mut parts, body) = req.into_parts();

        // Insert x-ms-version header for normal requests.
        parts.headers.insert(
            HeaderName::from_static(X_MS_VERSION),
            // consistent with azdls and azblob
            HeaderValue::from_static("2022-11-02"),
        );

        self.signer(ctx)
            .sign(&mut parts, None)
            .await
            .map_err(|e| new_request_sign_error(e.into()))?;

        Ok(Request::from_parts(parts, body))
    }

    #[inline]
    pub async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<Buffer>> {
        ctx.http_transport().send(req).await
    }

    pub async fn azfile_read(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<Response<HttpBody>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        let mut req = Request::get(&url);

        if !range.is_full() {
            req = req.header(RANGE, range.to_header());
        }

        let req = req
            .extension(Operation::Read)
            .extension(ServiceOperation("GetFile"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        ctx.http_transport().fetch(req).await
    }

    pub async fn azfile_create_file(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: usize,
        args: &OpWrite,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path)
            .trim_start_matches('/')
            .to_string();
        let url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        let mut req = Request::put(&url);

        // x-ms-content-length specifies the maximum size for the file, up to 4 tebibytes (TiB)
        // https://learn.microsoft.com/en-us/rest/api/storageservices/create-file
        req = req.header(X_MS_CONTENT_LENGTH, size);

        req = req.header(X_MS_TYPE, "file");

        // Content length must be 0 for create request.
        req = req.header(CONTENT_LENGTH, 0);

        if let Some(ty) = args.content_type() {
            req = req.header(CONTENT_TYPE, ty);
        }

        if let Some(pos) = args.content_disposition() {
            req = req.header(CONTENT_DISPOSITION, pos);
        }

        // Set user metadata headers.
        if let Some(user_metadata) = args.user_metadata() {
            for (key, value) in user_metadata {
                req = req.header(format!("{X_MS_META_PREFIX}{key}"), value);
            }
        }

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("CreateFile"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azfile_update(
        &self,
        ctx: &OperationContext,
        path: &str,
        size: u64,
        position: u64,
        body: Buffer,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path)
            .trim_start_matches('/')
            .to_string();

        let url = format!(
            "{}/{}/{}?comp=range",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        let mut req = Request::put(&url);

        req = req.header(CONTENT_LENGTH, size);

        req = req.header(X_MS_WRITE, "update");

        req = req.header(
            RANGE,
            BytesRange::from(position..position + size).to_header(),
        );

        let req = req
            .extension(Operation::Write)
            .extension(ServiceOperation("PutRange"));

        let req = req.body(body).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azfile_get_file_properties(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);
        let url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        let req = Request::head(&url);

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetFileProperties"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azfile_get_directory_properties(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path);

        let url = format!(
            "{}/{}/{}?restype=directory",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        let req = Request::head(&url);

        let req = req
            .extension(Operation::Stat)
            .extension(ServiceOperation("GetDirectoryProperties"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azfile_rename(
        &self,
        ctx: &OperationContext,
        path: &str,
        new_path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path)
            .trim_start_matches('/')
            .to_string();

        let new_p = build_abs_path(&self.root, new_path)
            .trim_start_matches('/')
            .to_string();

        let url = if path.ends_with('/') {
            format!(
                "{}/{}/{}?restype=directory&comp=rename",
                self.endpoint,
                self.share_name,
                percent_encode_path(&new_p)
            )
        } else {
            format!(
                "{}/{}/{}?comp=rename",
                self.endpoint,
                self.share_name,
                percent_encode_path(&new_p)
            )
        };

        let mut req = Request::put(&url);

        req = req.header(CONTENT_LENGTH, 0);

        // x-ms-file-rename-source specifies the file or directory to be renamed.
        // the value must be a URL style path
        // the official document does not mention the URL style path
        // find the solution from the community FAQ and implementation of the Java-SDK
        // ref: https://learn.microsoft.com/en-us/answers/questions/799611/azure-file-service-rest-api(rename)?page=1
        let source_url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        req = req.header(X_MS_FILE_RENAME_SOURCE, &source_url);

        req = req.header(X_MS_FILE_RENAME_REPLACE_IF_EXISTS, "true");

        let req = req
            .extension(Operation::Rename)
            .extension(ServiceOperation("Rename"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azfile_create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path)
            .trim_start_matches('/')
            .to_string();

        let url = format!(
            "{}/{}/{}?restype=directory",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        let mut req = Request::put(&url);

        req = req.header(CONTENT_LENGTH, 0);

        let req = req
            .extension(Operation::CreateDir)
            .extension(ServiceOperation("CreateDirectory"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azfile_delete_file(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path)
            .trim_start_matches('/')
            .to_string();

        let url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        let req = Request::delete(&url);

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteFile"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azfile_delete_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path)
            .trim_start_matches('/')
            .to_string();

        let url = format!(
            "{}/{}/{}?restype=directory",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p)
        );

        let req = Request::delete(&url);

        let req = req
            .extension(Operation::Delete)
            .extension(ServiceOperation("DeleteDirectory"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn azfile_list(
        &self,
        ctx: &OperationContext,
        path: &str,
        limit: &Option<usize>,
        continuation: &str,
    ) -> Result<Response<Buffer>> {
        let p = build_abs_path(&self.root, path)
            .trim_start_matches('/')
            .to_string();

        let url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.share_name,
            percent_encode_path(&p),
        );

        let mut url = QueryPairsWriter::new(&url)
            .push("restype", "directory")
            .push("comp", "list")
            .push("include", "Timestamps,ETag");

        if !continuation.is_empty() {
            url = url.push("marker", &percent_encode_path(continuation));
        }

        if let Some(limit) = limit {
            url = url.push("maxresults", &limit.to_string());
        }

        let req = Request::get(url.finish());

        let req = req
            .extension(Operation::List)
            .extension(ServiceOperation("ListDirectoriesAndFiles"));

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let req = self.sign(ctx, req).await?;
        self.send(ctx, req).await
    }

    pub async fn ensure_parent_dir_exists(&self, ctx: &OperationContext, path: &str) -> Result<()> {
        let mut dirs = VecDeque::default();
        // azure file service does not support recursive directory creation
        let mut p = path;
        while p != "/" {
            p = get_parent(p);
            dirs.push_front(p);
        }

        let mut pop_dir_count = dirs.len();
        for dir in dirs.iter().rev() {
            let resp = self.azfile_get_directory_properties(ctx, dir).await?;
            if resp.status() == StatusCode::NOT_FOUND {
                pop_dir_count -= 1;
                continue;
            }
            break;
        }

        for dir in dirs.iter().skip(pop_dir_count) {
            let resp = self.azfile_create_dir(ctx, dir).await?;

            if resp.status() == StatusCode::CREATED {
                continue;
            }

            if resp
                .headers()
                .get("x-ms-error-code")
                .map(|value| value.to_str().unwrap_or(""))
                .unwrap_or_else(|| "")
                == "ResourceAlreadyExists"
            {
                continue;
            }

            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateDirectory")),
                resp,
            ));
        }

        Ok(())
    }
}

use bytes::Buf;
use opendal_core::raw::ServiceOperation;
use opendal_service_azure_common::with_azure_error_response_context;
use quick_xml::de;
use serde::Deserialize;

/// AzfileError is the error returned by azure file service.
#[derive(Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct AzfileError {
    code: String,
    message: String,
    query_parameter_name: String,
    query_parameter_value: String,
    reason: String,
}

impl Debug for AzfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut de = f.debug_struct("AzfileError");
        de.field("code", &self.code);
        // replace `\n` to ` ` for better reading.
        de.field("message", &self.message.replace('\n', " "));

        if !self.query_parameter_name.is_empty() {
            de.field("query_parameter_name", &self.query_parameter_name);
        }
        if !self.query_parameter_value.is_empty() {
            de.field("query_parameter_value", &self.query_parameter_value);
        }
        if !self.reason.is_empty() {
            de.field("reason", &self.reason);
        }

        de.finish()
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

    let mut azfile_error = de::from_reader::<_, AzfileError>(bs.clone().reader()).ok();
    if azfile_error.as_ref().is_none_or(|err| err.code.is_empty())
        && let Some(code) = parts
            .headers
            .get("x-ms-error-code")
            .and_then(|value| value.to_str().ok())
    {
        azfile_error.get_or_insert_with(AzfileError::default).code = code.to_string();
    }

    let (mut kind, mut retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::TOO_MANY_REQUESTS => (ErrorKind::RateLimited, true),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    if let Some(azfile_error) = &azfile_error {
        match azfile_error.code.as_str() {
            "ParentNotFound" => {
                (kind, retryable) = (ErrorKind::NotFound, false);
            }
            "ResourceAlreadyExists" => {
                (kind, retryable) = (ErrorKind::AlreadyExists, false);
            }
            "DeletePending" | "ShareBeingDeleted"
                if ctx.service_operation == ServiceOperation("CreateDirectory") =>
            {
                (kind, retryable) = (ErrorKind::Conflict, true);
            }
            "CannotDeleteFileOrDirectory"
            | "ContainerQuotaDowngradeNotAllowed"
            | "DeletePending"
            | "DeleteShareWhenSnapshotLeased"
            | "DirectoryNotEmpty"
            | "FileGenerationMismatch"
            | "FileLockConflict"
            | "FileOpenBySmbClient"
            | "LeaseAcquireDuringShareDelete"
            | "LeaseIdMismatchWithFileLeaseOperation"
            | "LeaseIdMismatchWithFileOperation"
            | "LeaseIdMismatchWithFileShareLeaseOperation"
            | "LeaseIdMismatchWithFileShareOperation"
            | "LeaseIdMissingWithFileOperation"
            | "LeaseIdMissingWithFileShareOperation"
            | "LeaseLostWithFileOperation"
            | "LeaseLostWithFileShareOperation"
            | "LeaseNotPresentWithFileLeaseOperation"
            | "LeaseNotPresentWithFileOperation"
            | "LeaseNotPresentWithFileShareLeaseOperation"
            | "LeaseNotPresentWithFileShareOperation"
            | "PreviousSnapshotNotFound"
            | "PreviousSnapshotOperationNotSupported"
            | "ReadOnlyAttribute"
            | "RenameCannotOverwriteDestinationFile"
            | "RenameCycle"
            | "RenameDirectoryHasOpenFiles"
            | "ShareAlreadyExists"
            | "ShareBeingDeleted"
            | "ShareHasSnapshots"
            | "ShareSnapshotInProgress"
            | "SharingViolation" => {
                (kind, retryable) = (ErrorKind::Conflict, false);
            }
            _ if matches!(
                parts.status,
                StatusCode::CONFLICT | StatusCode::PRECONDITION_FAILED
            ) =>
            {
                (kind, retryable) = (ErrorKind::Unexpected, false);
            }
            _ => {}
        }
    }

    let message = azfile_error
        .map(|err| format!("{err:?}"))
        .unwrap_or_else(|| String::from_utf8_lossy(&bs).into_owned());

    let mut err = Error::new(kind, &message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_azure_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_native_error(operation: &'static str, status: StatusCode, code: &str) -> Error {
        let resp = Response::builder()
            .status(status)
            .header("x-ms-error-code", code)
            .body(Buffer::new())
            .expect("response must build");
        parse_error(ErrorContext::new(ServiceOperation(operation)), resp)
    }

    #[test]
    fn test_parse_native_conflict_codes() {
        let err = parse_native_error("CreateDirectory", StatusCode::CONFLICT, "DeletePending");
        assert_eq!(err.kind(), ErrorKind::Conflict);
        assert!(err.is_temporary());

        let err = parse_native_error("PutRange", StatusCode::CONFLICT, "FileLockConflict");
        assert_eq!(err.kind(), ErrorKind::Conflict);
        assert!(!err.is_temporary());

        let err = parse_native_error(
            "GetFileProperties",
            StatusCode::PRECONDITION_FAILED,
            "LeaseIdMismatchWithFileOperation",
        );
        assert_eq!(err.kind(), ErrorKind::Conflict);
        assert!(!err.is_temporary());
    }

    #[test]
    fn test_parse_non_conflict_native_codes() {
        assert_eq!(
            parse_native_error(
                "CreateDirectory",
                StatusCode::CONFLICT,
                "ResourceAlreadyExists"
            )
            .kind(),
            ErrorKind::AlreadyExists
        );
        assert_eq!(
            parse_native_error("CreateDirectory", StatusCode::NOT_FOUND, "ParentNotFound").kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            parse_native_error(
                "GetFileProperties",
                StatusCode::PRECONDITION_FAILED,
                "UnknownPrecondition"
            )
            .kind(),
            ErrorKind::Unexpected
        );
    }
}
