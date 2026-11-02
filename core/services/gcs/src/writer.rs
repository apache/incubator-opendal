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

use std::sync::Arc;

use bytes::Buf;
use http::StatusCode;
use http::header::LOCATION;
use http::header::RANGE;

use super::core::CompleteMultipartUploadRequestPart;
use super::core::ErrorContext;
use super::core::GcsCore;
use super::core::InitiateMultipartUploadResult;
use super::core::constants::X_GOOG_GENERATION;
use super::core::parse_error;
use opendal_core::raw::*;
use opendal_core::*;

pub type GcsWriters = TwoWays<oio::MultipartWriter<GcsWriter>, GcsConditionalWriter>;

const RESUMABLE_CHUNK_SIZE: usize = 8 * 1024 * 1024;

pub struct GcsWriter {
    core: Arc<GcsCore>,
    ctx: OperationContext,
    path: String,
    op: OpWrite,
}

impl GcsWriter {
    pub fn new(core: Arc<GcsCore>, ctx: OperationContext, path: &str, op: OpWrite) -> Self {
        GcsWriter {
            core,
            ctx,
            path: path.to_string(),
            op,
        }
    }
}

impl oio::MultipartWrite for GcsWriter {
    async fn write_once(&self, _: u64, body: Buffer) -> Result<Metadata> {
        let size = body.len() as u64;
        // Request builders own percent-encoding; writers pass logical paths unchanged.
        let req = self
            .core
            .gcs_insert_object_request(&self.path, Some(size), &self.op, body)?;

        let req = self.core.sign(&self.ctx, req).await?;

        let resp = self.core.send(&self.ctx, req).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => {
                let metadata =
                    GcsCore::build_metadata_from_object_response(&self.path, resp.into_body())?;
                Ok(metadata)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("InsertObject"))
                    .with_caller_condition(self.op.is_conditional()),
                resp,
            )),
        }
    }

    async fn initiate_part(&self) -> Result<String> {
        let resp = self
            .core
            .gcs_initiate_multipart_upload(&self.ctx, &self.path, &self.op)
            .await?;

        if !resp.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateMultipartUpload")),
                resp,
            ));
        }

        let buf = resp.into_body();
        let upload_id: InitiateMultipartUploadResult =
            quick_xml::de::from_reader(buf.reader()).map_err(new_xml_deserialize_error)?;
        Ok(upload_id.upload_id)
    }

    async fn write_part(
        &self,
        upload_id: &str,
        part_number: usize,
        size: u64,
        body: Buffer,
    ) -> Result<oio::MultipartPart> {
        // Gcs requires part number must between [1..=10000]
        let part_number = part_number + 1;

        let resp = self
            .core
            .gcs_upload_part(&self.ctx, &self.path, upload_id, part_number, size, body)
            .await?;

        if !resp.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadPart")),
                resp,
            ));
        }

        let etag = parse_etag(resp.headers())?
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "ETag not present in returning response",
                )
            })?
            .to_string();

        Ok(oio::MultipartPart {
            part_number,
            etag,
            checksum: None,
            size: None,
        })
    }

    async fn complete_part(
        &self,
        upload_id: &str,
        parts: &[oio::MultipartPart],
    ) -> Result<Metadata> {
        let parts = parts
            .iter()
            .map(|p| CompleteMultipartUploadRequestPart {
                part_number: p.part_number,
                etag: p.etag.clone(),
            })
            .collect();

        let resp = self
            .core
            .gcs_complete_multipart_upload(&self.ctx, &self.path, upload_id, parts)
            .await?;

        if !resp.status().is_success() {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CompleteMultipartUpload")),
                resp,
            ));
        }
        let mut metadata = if self.path.ends_with('/') {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::unknown()
        };
        if let Some(etag) = parse_etag(resp.headers())? {
            metadata.etag(etag);
        }
        if let Some(generation) = parse_header_to_str(resp.headers(), X_GOOG_GENERATION)? {
            metadata.version(generation);
        }
        Ok(metadata.build())
    }

    async fn abort_part(&self, upload_id: &str) -> Result<()> {
        let resp = self
            .core
            .gcs_abort_multipart_upload(&self.ctx, &self.path, upload_id)
            .await?;
        match resp.status() {
            // gcs returns code 204 if abort succeeds.
            StatusCode::NO_CONTENT => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("AbortMultipartUpload")),
                resp,
            )),
        }
    }
}

pub struct GcsConditionalWriter {
    core: Arc<GcsCore>,
    ctx: OperationContext,
    path: String,
    op: OpWrite,
    session_uri: Option<String>,
    offset: u64,
    pending: oio::QueueBuf,
    finalize_state: FinalizeState,
}

enum ResumableUploadStatus {
    Incomplete(u64),
    Complete(Box<Metadata>),
}

#[derive(Eq, PartialEq)]
enum FinalizeState {
    Ready,
    CheckStatus,
}

impl GcsConditionalWriter {
    pub fn new(core: Arc<GcsCore>, ctx: OperationContext, path: &str, op: OpWrite) -> Self {
        Self {
            core,
            ctx,
            path: path.to_string(),
            op,
            session_uri: None,
            offset: 0,
            pending: oio::QueueBuf::new(),
            finalize_state: FinalizeState::Ready,
        }
    }

    async fn ensure_session(&mut self) -> Result<&str> {
        if self.session_uri.is_none() {
            let resp = self
                .core
                .gcs_initiate_resumable_upload(&self.ctx, &self.path, &self.op)
                .await?;
            if !resp.status().is_success() {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("InitiateResumableUpload"))
                        .with_caller_condition(self.op.is_conditional()),
                    resp,
                ));
            }
            let location = parse_header_to_str(resp.headers(), LOCATION)?
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "GCS resumable upload response is missing Location",
                    )
                })?
                .to_string();
            self.session_uri = Some(location);
        }
        Ok(self.session_uri.as_deref().unwrap())
    }

    fn persisted_offset(resp: &http::Response<Buffer>) -> Result<u64> {
        let Some(range) = parse_header_to_str(resp.headers(), RANGE)? else {
            return Ok(0);
        };
        let end = range
            .strip_prefix("bytes=")
            .and_then(|v| v.rsplit_once('-'))
            .map(|(_, end)| end)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "invalid Range header in GCS resumable upload response",
                )
                .with_context("range", range)
            })?;
        end.parse::<u64>().map(|v| v + 1).map_err(|err| {
            Error::new(
                ErrorKind::Unexpected,
                "invalid Range header in GCS resumable upload response",
            )
            .with_context("range", range)
            .set_source(err)
        })
    }

    async fn query_status(&mut self, total: u64) -> Result<ResumableUploadStatus> {
        let session_uri = self.ensure_session().await?.to_string();
        let resp = self
            .core
            .gcs_query_resumable_upload(&self.ctx, &session_uri, total)
            .await?;

        match resp.status() {
            StatusCode::PERMANENT_REDIRECT => {
                let persisted = Self::persisted_offset(&resp)?;
                Ok(ResumableUploadStatus::Incomplete(persisted))
            }
            StatusCode::OK | StatusCode::CREATED => {
                let metadata =
                    GcsCore::build_metadata_from_object_response(&self.path, resp.into_body())?;
                Ok(ResumableUploadStatus::Complete(Box::new(metadata)))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("QueryResumableUpload")),
                resp,
            )),
        }
    }

    async fn upload_chunk(&mut self, body: Buffer) -> Result<()> {
        let start = self.offset;
        let end = start + body.len() as u64;
        let session_uri = self.ensure_session().await?.to_string();
        let resp = self
            .core
            .gcs_upload_resumable_chunk(&self.ctx, &session_uri, start, body, None)
            .await?;

        if resp.status() == StatusCode::PERMANENT_REDIRECT {
            let persisted = Self::persisted_offset(&resp)?;
            if persisted < start || persisted > end {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "GCS resumable upload reported invalid persisted range",
                )
                .with_context("start", start)
                .with_context("end", end)
                .with_context("persisted", persisted)
                .set_temporary());
            }
            if persisted < end {
                // GCS ignores an already persisted prefix when the same chunk
                // is retried, so advance only after the full chunk is acknowledged.
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "GCS resumable upload persisted only part of a chunk",
                )
                .with_context("start", start)
                .with_context("end", end)
                .with_context("persisted", persisted)
                .set_temporary());
            }
            self.offset = end;
            return Ok(());
        }

        if resp.status() == StatusCode::OK || resp.status() == StatusCode::CREATED {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "GCS finalized a resumable upload before the last chunk",
            ));
        }

        Err(parse_error(
            ErrorContext::new(ServiceOperation("UploadResumableChunk"))
                .with_caller_condition(self.op.is_conditional()),
            resp,
        ))
    }

    async fn flush_pending(&mut self) -> Result<()> {
        while self.pending.len() > RESUMABLE_CHUNK_SIZE {
            let mut chunk = self.pending.clone().collect();
            chunk.truncate(RESUMABLE_CHUNK_SIZE);
            self.upload_chunk(chunk).await?;
            self.pending.advance(RESUMABLE_CHUNK_SIZE);
        }
        Ok(())
    }
}

impl oio::Write for GcsConditionalWriter {
    async fn write(&mut self, body: Buffer) -> Result<()> {
        self.flush_pending().await?;
        self.pending.push(body);
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.flush_pending().await?;
        let body = self.pending.clone().collect();
        let start = self.offset;
        let total = start + body.len() as u64;

        if self.finalize_state == FinalizeState::CheckStatus {
            match self.query_status(total).await? {
                ResumableUploadStatus::Incomplete(persisted) => {
                    if persisted < start || persisted > total {
                        return Err(Error::new(
                            ErrorKind::Unexpected,
                            "GCS resumable upload reported invalid persisted range",
                        )
                        .with_context("start", start)
                        .with_context("end", total)
                        .with_context("persisted", persisted)
                        .set_temporary());
                    }
                    self.finalize_state = FinalizeState::Ready;
                }
                ResumableUploadStatus::Complete(metadata) => {
                    self.finalize_state = FinalizeState::Ready;
                    self.offset = total;
                    self.pending.advance(body.len());
                    return Ok(*metadata);
                }
            }
        }

        let session_uri = self.ensure_session().await?.to_string();
        self.finalize_state = FinalizeState::CheckStatus;
        let resp = self
            .core
            .gcs_upload_resumable_chunk(&self.ctx, &session_uri, start, body.clone(), Some(total))
            .await?;

        if resp.status() == StatusCode::PERMANENT_REDIRECT {
            let persisted = Self::persisted_offset(&resp)?;
            if persisted < start || persisted > total {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "GCS resumable upload reported invalid persisted range",
                )
                .with_context("start", start)
                .with_context("end", total)
                .with_context("persisted", persisted)
                .set_temporary());
            }
            self.finalize_state = FinalizeState::Ready;
            return Err(Error::new(
                ErrorKind::Unexpected,
                "GCS did not finalize the last resumable upload chunk",
            )
            .with_context("persisted", persisted)
            .set_temporary());
        }

        if resp.status() == StatusCode::OK || resp.status() == StatusCode::CREATED {
            let metadata =
                GcsCore::build_metadata_from_object_response(&self.path, resp.into_body())?;
            self.finalize_state = FinalizeState::Ready;
            self.offset = total;
            self.pending.advance(body.len());
            return Ok(metadata);
        }

        let err = parse_error(
            ErrorContext::new(ServiceOperation("UploadResumableChunk"))
                .with_caller_condition(self.op.is_conditional()),
            resp,
        );
        if !err.is_temporary() {
            self.finalize_state = FinalizeState::Ready;
        }
        Err(err)
    }

    async fn abort(&mut self) -> Result<()> {
        let Some(session_uri) = self.session_uri.as_deref() else {
            self.pending.clear();
            return Ok(());
        };
        let resp = self
            .core
            .gcs_cancel_resumable_upload(&self.ctx, session_uri)
            .await?;
        if resp.status().is_success()
            || resp.status().as_u16() == 499
            || resp.status() == StatusCode::NOT_FOUND
            || resp.status() == StatusCode::GONE
        {
            self.session_uri = None;
            self.pending.clear();
            Ok(())
        } else {
            Err(parse_error(
                ErrorContext::new(ServiceOperation("CancelResumableUpload")),
                resp,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use bytes::Bytes;
    use futures::stream;
    use http::Request;
    use http::Response;
    use http::header::CONTENT_LENGTH;
    use opendal_core::raw::oio::Write as _;
    use reqsign_core::Context;
    use reqsign_core::ProvideCredentialChain;
    use reqsign_core::Signer;
    use reqsign_google::RequestSigner;
    use reqsign_google::TokenCredentialProvider;

    use super::*;

    #[derive(Default)]
    struct MockState {
        uploaded: Vec<u8>,
        ranges: Vec<String>,
        fail_chunk_response: bool,
        fail_final_response: bool,
        fail_final_response_partially: bool,
        completed: bool,
    }

    #[derive(Clone, Default)]
    struct MockResumableTransport {
        state: Arc<Mutex<MockState>>,
    }

    impl MockResumableTransport {
        fn fail_chunk_response(&self) {
            self.state.lock().unwrap().fail_chunk_response = true;
        }

        fn fail_final_response(&self) {
            self.state.lock().unwrap().fail_final_response = true;
        }

        fn fail_final_response_partially(&self) {
            self.state.lock().unwrap().fail_final_response_partially = true;
        }

        fn uploaded(&self) -> Vec<u8> {
            self.state.lock().unwrap().uploaded.clone()
        }

        fn ranges(&self) -> Vec<String> {
            self.state.lock().unwrap().ranges.clone()
        }

        fn response(
            status: StatusCode,
            range: Option<String>,
            body: Option<String>,
        ) -> Result<Response<HttpBody>> {
            let body = body.unwrap_or_default();
            let size = body.len() as u64;
            let mut response = Response::builder()
                .status(status)
                .header(CONTENT_LENGTH, size);
            if let Some(range) = range {
                response = response.header(RANGE, range);
            }
            let stream = if body.is_empty() {
                stream::iter(Vec::<Result<Buffer>>::new())
            } else {
                stream::iter(vec![Ok(Buffer::from(Bytes::from(body)))])
            };
            Ok(response
                .body(HttpBody::new(stream, Some(size)))
                .expect("mock response must build"))
        }

        fn completed_response(size: usize) -> Result<Response<HttpBody>> {
            Self::response(
                StatusCode::OK,
                None,
                Some(format!(r#"{{"size":"{size}","generation":"1"}}"#)),
            )
        }
    }

    impl HttpTransport for MockResumableTransport {
        async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
            if req.method() == http::Method::POST {
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(LOCATION, "https://upload.example/session")
                    .header(CONTENT_LENGTH, 0)
                    .body(HttpBody::new(
                        stream::iter(Vec::<Result<Buffer>>::new()),
                        Some(0),
                    ))
                    .expect("mock response must build"));
            }

            assert_eq!(req.method(), http::Method::PUT);
            assert_eq!(req.uri(), "https://upload.example/session");

            let content_range = req
                .headers()
                .get("content-range")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let body = req.body().to_bytes();
            let mut state = self.state.lock().unwrap();
            state.ranges.push(content_range.clone());

            if body.is_empty() {
                if content_range == "bytes */0" && state.uploaded.is_empty() {
                    state.completed = true;
                    return Self::completed_response(0);
                }
                if state.completed {
                    return Self::completed_response(state.uploaded.len());
                }
                let range = (!state.uploaded.is_empty())
                    .then(|| format!("bytes=0-{}", state.uploaded.len() - 1));
                return Self::response(StatusCode::PERMANENT_REDIRECT, range, None);
            }

            let value = content_range.strip_prefix("bytes ").unwrap();
            let (range, total) = value.split_once('/').unwrap();
            let (start, end) = range.split_once('-').unwrap();
            let start = start.parse::<usize>().unwrap();
            let end = end.parse::<usize>().unwrap();
            assert_eq!(end + 1 - start, body.len());
            assert!(start <= state.uploaded.len());
            let persisted = state.uploaded.len() - start;
            assert!(persisted <= body.len());
            assert_eq!(&state.uploaded[start..], &body[..persisted]);
            let remaining = &body[persisted..];

            if total == "*" {
                if state.fail_chunk_response {
                    state.fail_chunk_response = false;
                    state
                        .uploaded
                        .extend_from_slice(&remaining[..remaining.len() / 2]);
                    return Err(
                        Error::new(ErrorKind::Unexpected, "mock lost chunk response")
                            .set_temporary(),
                    );
                }
                state.uploaded.extend_from_slice(remaining);
                let range = Some(format!("bytes=0-{}", state.uploaded.len() - 1));
                return Self::response(StatusCode::PERMANENT_REDIRECT, range, None);
            }

            if state.fail_final_response_partially {
                state.fail_final_response_partially = false;
                state
                    .uploaded
                    .extend_from_slice(&remaining[..remaining.len() / 2]);
                return Err(
                    Error::new(ErrorKind::Unexpected, "mock lost partial final response")
                        .set_temporary(),
                );
            }
            state.uploaded.extend_from_slice(remaining);
            assert_eq!(total.parse::<usize>().unwrap(), state.uploaded.len());
            state.completed = true;
            if state.fail_final_response {
                state.fail_final_response = false;
                return Err(
                    Error::new(ErrorKind::Unexpected, "mock lost final response").set_temporary(),
                );
            }
            Self::completed_response(state.uploaded.len())
        }
    }

    fn test_writer(transport: MockResumableTransport) -> GcsConditionalWriter {
        let sign_ctx = Context::new();
        let signer = Signer::new(
            sign_ctx.clone(),
            ProvideCredentialChain::new().push(TokenCredentialProvider::new("test-token")),
            RequestSigner::new("storage"),
        );
        let core = Arc::new(GcsCore {
            info: ServiceInfo::new("gcs", "/", "test-bucket"),
            capability: Capability::default(),
            endpoint: "https://storage.googleapis.com".to_string(),
            bucket: "test-bucket".to_string(),
            root: "/".to_string(),
            signer,
            sign_ctx,
            predefined_acl: None,
            default_storage_class: None,
            skip_signature: true,
        });
        let ctx = OperationContext::new().with_http_transport(HttpTransporter::new(transport));
        let (op, _) = OpWrite::from_options(
            &Capability::default(),
            options::WriteOptions {
                if_not_exists: true,
                ..Default::default()
            },
        )
        .unwrap();
        GcsConditionalWriter::new(core, ctx, "object", op)
    }

    #[tokio::test]
    async fn retry_write_resends_interrupted_chunk_before_accepting_input() {
        let transport = MockResumableTransport::default();
        transport.fail_chunk_response();
        let mut writer = test_writer(transport.clone());
        let mut expected = vec![7; RESUMABLE_CHUNK_SIZE + 3];
        let body = Buffer::from(Bytes::copy_from_slice(&expected));
        let next = Bytes::from_static(b"next");

        writer.write(body).await.unwrap();
        let err = writer.write(Buffer::from(next.clone())).await.unwrap_err();
        assert!(err.is_temporary());
        writer.write(Buffer::from(next.clone())).await.unwrap();
        let metadata = writer.close().await.unwrap();

        expected.extend_from_slice(&next);
        assert_eq!(transport.uploaded(), expected);
        assert_eq!(
            metadata.content_length(),
            (RESUMABLE_CHUNK_SIZE + 3 + next.len()) as u64
        );
        assert_eq!(
            transport.ranges(),
            vec![
                format!("bytes 0-{}/*", RESUMABLE_CHUNK_SIZE - 1),
                format!("bytes 0-{}/*", RESUMABLE_CHUNK_SIZE - 1),
                format!(
                    "bytes {}-{}/{}",
                    RESUMABLE_CHUNK_SIZE,
                    RESUMABLE_CHUNK_SIZE + 2 + next.len(),
                    RESUMABLE_CHUNK_SIZE + 3 + next.len()
                ),
            ]
        );
    }

    #[tokio::test]
    async fn retry_close_reconciles_lost_final_response() {
        let transport = MockResumableTransport::default();
        transport.fail_final_response();
        let mut writer = test_writer(transport.clone());
        let expected = b"hello".to_vec();
        writer
            .write(Buffer::from(Bytes::copy_from_slice(&expected)))
            .await
            .unwrap();

        let err = writer.close().await.unwrap_err();
        assert!(err.is_temporary());
        let metadata = writer.close().await.unwrap();

        assert_eq!(transport.uploaded(), expected);
        assert_eq!(metadata.content_length(), 5);
        assert_eq!(
            transport.ranges(),
            vec!["bytes 0-4/5".to_string(), "bytes */5".to_string()]
        );
    }

    #[tokio::test]
    async fn retry_close_resends_after_incomplete_status() {
        let transport = MockResumableTransport::default();
        transport.fail_final_response_partially();
        let mut writer = test_writer(transport.clone());
        let expected = b"hello".to_vec();
        writer
            .write(Buffer::from(Bytes::copy_from_slice(&expected)))
            .await
            .unwrap();

        let err = writer.close().await.unwrap_err();
        assert!(err.is_temporary());
        let metadata = writer.close().await.unwrap();

        assert_eq!(transport.uploaded(), expected);
        assert_eq!(metadata.content_length(), 5);
        assert_eq!(
            transport.ranges(),
            vec![
                "bytes 0-4/5".to_string(),
                "bytes */5".to_string(),
                "bytes 0-4/5".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn close_empty_object_uses_resumable_upload() {
        let transport = MockResumableTransport::default();
        let mut writer = test_writer(transport.clone());

        let metadata = writer.close().await.unwrap();

        assert_eq!(metadata.content_length(), 0);
        assert_eq!(transport.uploaded(), Vec::<u8>::new());
        assert_eq!(transport.ranges(), vec!["bytes */0".to_string()]);
    }
}
