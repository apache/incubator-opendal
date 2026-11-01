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

use std::fmt::{Debug, Formatter};
use std::sync::OnceLock;

use http::header::AUTHORIZATION;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqsign_core::{Context, Signer};
use reqsign_google::Credential;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint};

use opendal_core::raw::*;
use opendal_core::*;

use crate::generated::google::storage::v2::Object;
use crate::generated::google::storage::v2::storage_client::StorageClient;

const MAX_GRPC_DECODING_MESSAGE_SIZE: usize = i32::MAX as usize;

pub(crate) struct GcsGrpcCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub endpoint: String,
    pub bucket: String,
    pub root: String,
    pub channel_endpoint: Endpoint,
    pub channel: OnceLock<Channel>,
    pub signer: Signer<Credential>,
    pub sign_ctx: Context,
    pub skip_signature: bool,
}

impl Debug for GcsGrpcCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsGrpcCore")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl GcsGrpcCore {
    pub fn bucket_resource(&self) -> String {
        format!("projects/_/buckets/{}", self.bucket)
    }

    pub fn object_name(&self, path: &str) -> String {
        build_abs_path(&self.root, path)
    }

    pub fn client(&self) -> StorageClient<Channel> {
        let channel = self
            .channel
            .get_or_init(|| self.channel_endpoint.connect_lazy())
            .clone();
        StorageClient::new(channel).max_decoding_message_size(MAX_GRPC_DECODING_MESSAGE_SIZE)
    }

    pub async fn request<T>(
        &self,
        ctx: &OperationContext,
        message: T,
        routing_parameters: &[(&str, &str)],
    ) -> Result<tonic::Request<T>> {
        let mut request = tonic::Request::new(message);
        if let Some(routing) = build_routing_header(routing_parameters) {
            request.metadata_mut().insert(
                "x-goog-request-params",
                MetadataValue::try_from(routing.as_str()).map_err(|err| {
                    Error::new(ErrorKind::Unexpected, "invalid gRPC routing metadata")
                        .set_source(err)
                })?,
            );
        }

        if self.skip_signature {
            return Ok(request);
        }

        let req = http::Request::get(&self.endpoint)
            .body(())
            .map_err(new_request_build_error)?;
        let (mut parts, _) = req.into_parts();
        let signer = self.signer.clone().with_context(
            self.sign_ctx
                .clone()
                .with_http_send(ctx.http_transport().clone()),
        );
        signer
            .sign(&mut parts, None)
            .await
            .map_err(|err| new_request_sign_error(err.into()))?;

        let authorization = parts.headers.get(AUTHORIZATION).ok_or_else(|| {
            Error::new(
                ErrorKind::PermissionDenied,
                "Google credential signer did not produce an authorization header",
            )
        })?;
        let mut authorization = MetadataValue::try_from(authorization.to_str().map_err(|err| {
            Error::new(
                ErrorKind::Unexpected,
                "authorization header is not valid ASCII",
            )
            .set_source(err)
        })?)
        .map_err(|err| {
            Error::new(
                ErrorKind::Unexpected,
                "authorization header is not valid gRPC metadata",
            )
            .set_source(err)
        })?;
        authorization.set_sensitive(true);
        request
            .metadata_mut()
            .insert("authorization", authorization);

        Ok(request)
    }
}

fn build_routing_header(parameters: &[(&str, &str)]) -> Option<String> {
    (!parameters.is_empty()).then(|| {
        parameters
            .iter()
            .map(|(key, value)| format!("{key}={}", utf8_percent_encode(value, NON_ALPHANUMERIC)))
            .collect::<Vec<_>>()
            .join("&")
    })
}

#[derive(Clone, Copy)]
pub(crate) struct ErrorContext {
    service_operation: ServiceOperation,
    if_not_exists: bool,
}

impl ErrorContext {
    pub(crate) const fn new(service_operation: ServiceOperation) -> Self {
        Self {
            service_operation,
            if_not_exists: false,
        }
    }

    pub(crate) const fn with_if_not_exists(mut self, if_not_exists: bool) -> Self {
        self.if_not_exists = if_not_exists;
        self
    }
}

pub(crate) fn parse_status(ctx: ErrorContext, status: tonic::Status) -> Error {
    use tonic::Code;

    let kind = match status.code() {
        Code::NotFound => ErrorKind::NotFound,
        Code::AlreadyExists if ctx.if_not_exists => ErrorKind::ConditionNotMatch,
        Code::AlreadyExists => ErrorKind::AlreadyExists,
        Code::PermissionDenied | Code::Unauthenticated => ErrorKind::PermissionDenied,
        Code::FailedPrecondition if ctx.if_not_exists => ErrorKind::ConditionNotMatch,
        Code::FailedPrecondition | Code::Aborted => ErrorKind::Conflict,
        Code::OutOfRange => ErrorKind::RangeNotSatisfied,
        Code::ResourceExhausted => ErrorKind::RateLimited,
        _ => ErrorKind::Unexpected,
    };
    let temporary = matches!(
        status.code(),
        Code::Cancelled
            | Code::DeadlineExceeded
            | Code::Internal
            | Code::ResourceExhausted
            | Code::Unavailable
            | Code::Unknown
    );
    let mut err = Error::new(kind, status.message().to_string())
        .with_context("grpc_code", status.code().description())
        .with_context("service_operation", ctx.service_operation.0);
    if temporary {
        err = err.set_temporary();
    }
    err
}

pub(crate) fn parse_generation(value: Option<&str>) -> Result<i64> {
    value
        .map(|v| {
            v.parse::<i64>().map_err(|err| {
                Error::new(ErrorKind::Unexpected, "GCS generation must be an integer")
                    .with_context("generation", v)
                    .set_source(err)
            })
        })
        .transpose()
        .map(|v| v.unwrap_or_default())
}

pub(crate) fn parse_object(object: &Object) -> Metadata {
    let mode = if object.name.ends_with('/') {
        EntryMode::DIR
    } else {
        EntryMode::FILE
    };
    let mut metadata = if mode == EntryMode::FILE {
        MetadataBuilder::file(object.size.max(0) as u64)
    } else {
        MetadataBuilder::dir()
    };
    if !object.cache_control.is_empty() {
        metadata.cache_control(&object.cache_control);
    }
    if !object.content_type.is_empty() {
        metadata.content_type(&object.content_type);
    }
    if !object.content_encoding.is_empty() {
        metadata.content_encoding(&object.content_encoding);
    }
    if !object.content_disposition.is_empty() {
        metadata.content_disposition(&object.content_disposition);
    }
    if !object.etag.is_empty() {
        metadata.etag(&object.etag);
    }
    if object.generation != 0 {
        metadata.version(object.generation.to_string());
    }
    if !object.metadata.is_empty() {
        metadata.user_metadata(object.metadata.clone());
    }
    if let Some(ts) = object.update_time.as_ref().or(object.create_time.as_ref())
        && let Ok(ts) = Timestamp::new(ts.seconds, ts.nanos)
    {
        metadata.last_modified(ts);
    }
    metadata.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_header_uses_binding_names() {
        let bucket = "projects/_/buckets/example-bucket";

        assert_eq!(
            build_routing_header(&[("bucket", bucket)]).as_deref(),
            Some("bucket=projects%2F%5F%2Fbuckets%2Fexample%2Dbucket")
        );
        assert_eq!(
            build_routing_header(&[("source_bucket", bucket), ("bucket", bucket)]).as_deref(),
            Some(
                "source_bucket=projects%2F%5F%2Fbuckets%2Fexample%2Dbucket&bucket=projects%2F%5F%2Fbuckets%2Fexample%2Dbucket"
            )
        );
    }

    #[test]
    fn status_classification_uses_operation_context() {
        let caller = ErrorContext::new(ServiceOperation("WriteObject")).with_if_not_exists(true);
        let err = parse_status(
            caller,
            tonic::Status::failed_precondition("generation mismatch"),
        );
        assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
        assert!(!err.is_temporary());

        let state_conflict = ErrorContext::new(ServiceOperation("QueryWriteStatus"));
        let err = parse_status(
            state_conflict,
            tonic::Status::failed_precondition("upload is not active"),
        );
        assert_eq!(err.kind(), ErrorKind::Conflict);
        assert!(!err.is_temporary());

        let concurrent = ErrorContext::new(ServiceOperation("RewriteObject"));
        let err = parse_status(concurrent, tonic::Status::aborted("concurrent update"));
        assert_eq!(err.kind(), ErrorKind::Conflict);
        assert!(!err.is_temporary());
    }
}
