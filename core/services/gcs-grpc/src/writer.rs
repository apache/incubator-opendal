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

use futures::stream;

use opendal_core::raw::*;
use opendal_core::*;

use crate::core::{ErrorContext, GcsGrpcCore, parse_object, parse_status};
use crate::generated::google::storage::v2::*;

const WRITE_CHUNK_ALIGNMENT: usize = 256 * 1024;
const MAX_WRITE_CHUNK_SIZE: usize = 2 * 1024 * 1024;

pub(super) struct GcsGrpcWriter {
    core: Arc<GcsGrpcCore>,
    ctx: OperationContext,
    path: String,
    args: OpWrite,
    buffer: Option<Buffer>,
    upload_id: Option<String>,
    write_offset: i64,
    pending: Option<PendingUpload>,
    state: GcsGrpcWriterState,
}

impl GcsGrpcWriter {
    pub(super) fn new(
        core: Arc<GcsGrpcCore>,
        ctx: OperationContext,
        path: &str,
        args: OpWrite,
    ) -> Self {
        Self {
            core,
            ctx,
            path: path.to_string(),
            args,
            buffer: None,
            upload_id: None,
            write_offset: 0,
            pending: None,
            state: GcsGrpcWriterState::Open,
        }
    }

    fn error_context(&self, service_operation: ServiceOperation) -> ErrorContext {
        ErrorContext::new(service_operation).with_if_not_exists(self.args.if_not_exists())
    }

    fn ensure_open(&self) -> Result<()> {
        match self.state {
            GcsGrpcWriterState::Open => Ok(()),
            GcsGrpcWriterState::Closed => Err(Error::new(
                ErrorKind::Unexpected,
                "writer is already closed",
            )),
            GcsGrpcWriterState::Aborted => Err(Error::new(
                ErrorKind::Unexpected,
                "writer is already aborted",
            )),
        }
    }

    fn write_spec(&self) -> WriteObjectSpec {
        WriteObjectSpec {
            resource: Some(Object {
                name: self.core.object_name(&self.path),
                bucket: self.core.bucket_resource(),
                content_type: self.args.content_type().unwrap_or_default().to_string(),
                content_disposition: self
                    .args
                    .content_disposition()
                    .unwrap_or_default()
                    .to_string(),
                content_encoding: self.args.content_encoding().unwrap_or_default().to_string(),
                cache_control: self.args.cache_control().unwrap_or_default().to_string(),
                metadata: self
                    .args
                    .user_metadata()
                    .map(|metadata| {
                        metadata
                            .into_iter()
                            .map(|(key, value)| (key.to_owned(), value.to_owned()))
                            .collect()
                    })
                    .unwrap_or_default(),
                ..Default::default()
            }),
            if_generation_match: self.args.if_not_exists().then_some(0),
        }
    }

    async fn start_resumable_write(&mut self) -> Result<()> {
        if self.upload_id.is_some() {
            return Ok(());
        }

        let bucket = self.core.bucket_resource();
        let request = StartResumableWriteRequest {
            write_object_spec: Some(self.write_spec()),
        };
        let request = self
            .core
            .request(&self.ctx, request, &[("bucket", &bucket)])
            .await?;
        let response = self
            .core
            .client()
            .start_resumable_write(request)
            .await
            .map_err(|status| {
                parse_status(
                    self.error_context(ServiceOperation("StartResumableWrite")),
                    status,
                )
            })?
            .into_inner();
        if response.upload_id.is_empty() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "GCS started a resumable write without an upload ID",
            ));
        }
        self.upload_id = Some(response.upload_id);
        Ok(())
    }

    async fn write_once(&self, body: Buffer) -> Result<Object> {
        let first_message = write_object_request::FirstMessage::WriteObjectSpec(self.write_spec());
        let requests = write_object_requests(body, 0, true, first_message)?;
        let bucket = self.core.bucket_resource();
        let request = self
            .core
            .request(&self.ctx, requests, &[("bucket", &bucket)])
            .await?;
        let response = self
            .core
            .client()
            .write_object(request)
            .await
            .map_err(|status| {
                parse_status(self.error_context(ServiceOperation("WriteObject")), status)
            })?
            .into_inner();
        match response.write_status {
            Some(write_object_response::WriteStatus::Resource(object)) => Ok(object),
            _ => Err(Error::new(
                ErrorKind::Unexpected,
                "GCS write completed without object metadata",
            )),
        }
    }

    async fn finish_pending(&mut self) -> Result<Option<Object>> {
        let outcome = self.drive_pending().await?;
        let pending = self
            .pending
            .take()
            .expect("pending upload must exist while finishing");
        match outcome {
            UploadOutcome::Persisted(size) if !pending.finish_write => {
                self.write_offset = size;
                self.buffer = Some(pending.tail);
                Ok(None)
            }
            UploadOutcome::Finalized(object) if pending.finish_write => {
                self.write_offset = pending.end_offset()?;
                self.buffer = None;
                self.upload_id = None;
                self.state = GcsGrpcWriterState::Closed;
                Ok(Some(*object))
            }
            UploadOutcome::Persisted(_) => Err(Error::new(
                ErrorKind::Unexpected,
                "GCS write did not finalize the object",
            )),
            UploadOutcome::Finalized(_) => Err(Error::new(
                ErrorKind::Unexpected,
                "GCS finalized an object before close",
            )),
        }
    }

    async fn drive_pending(&mut self) -> Result<UploadOutcome> {
        loop {
            if self
                .pending
                .as_ref()
                .expect("pending upload must exist")
                .in_flight
            {
                match self.query_write_status().await? {
                    QueryOutcome::NotFound => {
                        let pending = self.pending.as_mut().expect("pending upload must exist");
                        if pending.write_offset != 0 {
                            return Err(Error::new(
                                ErrorKind::Unexpected,
                                "GCS resumable upload disappeared after data was persisted",
                            ));
                        }
                        pending.in_flight = false;
                    }
                    QueryOutcome::Persisted(size) => {
                        if let Some(outcome) = self.reconcile_persisted_size(size)? {
                            return Ok(outcome);
                        }
                    }
                    QueryOutcome::Finalized(object) => {
                        if self
                            .pending
                            .as_ref()
                            .expect("pending upload must exist")
                            .finish_write
                        {
                            return Ok(UploadOutcome::Finalized(object));
                        }
                        return Err(Error::new(
                            ErrorKind::Unexpected,
                            "GCS finalized an object before close",
                        ));
                    }
                }
            }

            let pending = self.pending.as_ref().expect("pending upload must exist");
            let body = pending.body.clone();
            let write_offset = pending.write_offset;
            let finish_write = pending.finish_write;
            let upload_id = self
                .upload_id
                .clone()
                .expect("resumable upload ID must exist");
            self.pending
                .as_mut()
                .expect("pending upload must exist")
                .in_flight = true;

            let first_message = write_object_request::FirstMessage::UploadId(upload_id);
            let requests =
                write_object_requests(body.clone(), write_offset, finish_write, first_message)?;
            let bucket = self.core.bucket_resource();
            let request = self
                .core
                .request(&self.ctx, requests, &[("bucket", &bucket)])
                .await?;
            let response = self
                .core
                .client()
                .write_object(request)
                .await
                .map_err(|status| {
                    parse_status(self.error_context(ServiceOperation("WriteObject")), status)
                })?
                .into_inner();

            match response.write_status {
                Some(write_object_response::WriteStatus::PersistedSize(size)) => {
                    if finish_write && body.is_empty() {
                        return Err(Error::new(
                            ErrorKind::Unexpected,
                            "GCS did not finalize an empty final write",
                        ));
                    }
                    if let Some(outcome) = self.reconcile_persisted_size(size)? {
                        return Ok(outcome);
                    }
                }
                Some(write_object_response::WriteStatus::Resource(object)) => {
                    return Ok(UploadOutcome::Finalized(Box::new(object)));
                }
                None => {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        "GCS write returned no persisted size or object metadata",
                    ));
                }
            }
        }
    }

    async fn query_write_status(&self) -> Result<QueryOutcome> {
        let upload_id = self
            .upload_id
            .as_deref()
            .expect("resumable upload ID must exist");
        let request = QueryWriteStatusRequest {
            upload_id: upload_id.to_string(),
        };
        let bucket = self.core.bucket_resource();
        let request = self
            .core
            .request(&self.ctx, request, &[("bucket", &bucket)])
            .await?;
        let response = match self.core.client().query_write_status(request).await {
            Ok(response) => response.into_inner(),
            Err(status) if status.code() == tonic::Code::NotFound => {
                return Ok(QueryOutcome::NotFound);
            }
            Err(status) => {
                return Err(parse_status(
                    ErrorContext::new(ServiceOperation("QueryWriteStatus")),
                    status,
                ));
            }
        };
        match response.write_status {
            Some(query_write_status_response::WriteStatus::PersistedSize(size)) => {
                Ok(QueryOutcome::Persisted(size))
            }
            Some(query_write_status_response::WriteStatus::Resource(object)) => {
                Ok(QueryOutcome::Finalized(Box::new(object)))
            }
            None => Err(Error::new(
                ErrorKind::Unexpected,
                "GCS query returned no persisted size or object metadata",
            )),
        }
    }

    fn reconcile_persisted_size(&mut self, size: i64) -> Result<Option<UploadOutcome>> {
        let pending = self.pending.as_mut().expect("pending upload must exist");
        let end_offset = pending.end_offset()?;
        if size < pending.write_offset || size > end_offset {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "GCS returned a persisted size outside the pending write range",
            )
            .with_context("write_offset", pending.write_offset.to_string())
            .with_context("end_offset", end_offset.to_string())
            .with_context("persisted_size", size.to_string()));
        }

        let consumed = usize::try_from(size - pending.write_offset).map_err(|err| {
            Error::new(ErrorKind::Unexpected, "GCS persisted size exceeds usize").set_source(err)
        })?;
        if consumed != 0 {
            pending.body = pending.body.slice(consumed..);
            pending.write_offset = size;
        }
        pending.in_flight = false;

        if pending.body.is_empty() && !pending.finish_write {
            Ok(Some(UploadOutcome::Persisted(size)))
        } else {
            Ok(None)
        }
    }
}

impl oio::Write for GcsGrpcWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.ensure_open()?;

        if let Some(pending) = self.pending.as_ref() {
            if pending.finish_write {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "writer is already finishing",
                ));
            }
            self.finish_pending().await?;
            return Ok(());
        }

        if self.buffer.is_none() && self.upload_id.is_none() {
            self.buffer = Some(bs);
            return Ok(());
        }

        self.start_resumable_write().await?;

        let combined = merge_buffers(self.buffer.clone().unwrap_or_default(), bs);
        let upload_size = combined.len() / WRITE_CHUNK_ALIGNMENT * WRITE_CHUNK_ALIGNMENT;
        if upload_size == 0 {
            self.buffer = Some(combined);
            return Ok(());
        }

        let mut body = combined;
        let tail = body.split_off(upload_size);
        self.pending = Some(PendingUpload {
            body,
            tail,
            write_offset: self.write_offset,
            finish_write: false,
            in_flight: false,
        });
        self.finish_pending().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.ensure_open()?;

        if self.pending.is_some()
            && let Some(object) = self.finish_pending().await?
        {
            return Ok(parse_object(&object));
        }

        if self.upload_id.is_none() {
            let body = self.buffer.clone().unwrap_or_default();
            let object = self.write_once(body).await?;
            self.buffer = None;
            self.state = GcsGrpcWriterState::Closed;
            return Ok(parse_object(&object));
        }

        self.pending = Some(PendingUpload {
            body: self.buffer.clone().unwrap_or_default(),
            tail: Buffer::new(),
            write_offset: self.write_offset,
            finish_write: true,
            in_flight: false,
        });
        let object = self.finish_pending().await?.ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "GCS write finalized without object metadata",
            )
        })?;
        Ok(parse_object(&object))
    }

    async fn abort(&mut self) -> Result<()> {
        self.ensure_open()?;
        if let Some(upload_id) = self.upload_id.as_deref() {
            let request = CancelResumableWriteRequest {
                upload_id: upload_id.to_string(),
            };
            let bucket = self.core.bucket_resource();
            let request = self
                .core
                .request(&self.ctx, request, &[("bucket", &bucket)])
                .await?;
            match self.core.client().cancel_resumable_write(request).await {
                Ok(_) => {}
                Err(status) if status.code() == tonic::Code::NotFound => {}
                Err(status) => {
                    return Err(parse_status(
                        ErrorContext::new(ServiceOperation("CancelResumableWrite")),
                        status,
                    ));
                }
            }
        }
        self.buffer = None;
        self.upload_id = None;
        self.pending = None;
        self.state = GcsGrpcWriterState::Aborted;
        Ok(())
    }
}

struct PendingUpload {
    body: Buffer,
    tail: Buffer,
    write_offset: i64,
    finish_write: bool,
    in_flight: bool,
}

impl PendingUpload {
    fn end_offset(&self) -> Result<i64> {
        self.write_offset
            .checked_add(i64::try_from(self.body.len()).map_err(|err| {
                Error::new(ErrorKind::Unexpected, "GCS write body exceeds i64").set_source(err)
            })?)
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "GCS write offset exceeds i64"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GcsGrpcWriterState {
    Open,
    Closed,
    Aborted,
}

enum UploadOutcome {
    Persisted(i64),
    Finalized(Box<Object>),
}

enum QueryOutcome {
    NotFound,
    Persisted(i64),
    Finalized(Box<Object>),
}

fn merge_buffers(left: Buffer, right: Buffer) -> Buffer {
    left.into_iter().chain(right).collect()
}

struct WriteObjectRequestState {
    body: Buffer,
    write_offset: i64,
    finish_write: bool,
    first_message: Option<write_object_request::FirstMessage>,
    emitted: bool,
}

fn write_object_requests(
    body: Buffer,
    write_offset: i64,
    finish_write: bool,
    first_message: write_object_request::FirstMessage,
) -> Result<impl futures::Stream<Item = WriteObjectRequest> + Send + 'static> {
    if !finish_write && !body.len().is_multiple_of(WRITE_CHUNK_ALIGNMENT) {
        return Err(Error::new(
            ErrorKind::Unexpected,
            "non-final GCS writes must be aligned to 256 KiB",
        ));
    }
    write_offset
        .checked_add(i64::try_from(body.len()).map_err(|err| {
            Error::new(ErrorKind::Unexpected, "GCS write body exceeds i64").set_source(err)
        })?)
        .ok_or_else(|| Error::new(ErrorKind::Unexpected, "GCS write offset exceeds i64"))?;

    Ok(stream::unfold(
        WriteObjectRequestState {
            body,
            write_offset,
            finish_write,
            first_message: Some(first_message),
            emitted: false,
        },
        |mut state| async move {
            if state.emitted && state.body.is_empty() {
                return None;
            }

            state.emitted = true;
            let (content, content_len) = if state.body.is_empty() {
                (None, 0)
            } else {
                let size = state.body.len().min(MAX_WRITE_CHUNK_SIZE);
                (Some(state.body.split_to(size).to_bytes()), size)
            };
            let request = WriteObjectRequest {
                write_offset: state.write_offset,
                finish_write: state.finish_write && state.body.is_empty(),
                first_message: state.first_message.take(),
                data: content.map(|content| {
                    write_object_request::Data::ChecksummedData(ChecksummedData { content })
                }),
            };
            if content_len != 0 {
                state.write_offset = state
                    .write_offset
                    .checked_add(
                        i64::try_from(content_len)
                            .expect("validated GCS write chunk size must fit in i64"),
                    )
                    .expect("validated GCS write offset must fit in i64");
            }
            Some((request, state))
        },
    ))
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures::StreamExt;
    use futures::executor::block_on;
    use opendal_core::raw::oio::Write as _;
    use reqsign_core::{Context, ProvideCredentialChain, Signer};
    use reqsign_google::RequestSigner;
    use tonic::transport::Endpoint;

    use super::*;
    use crate::GCS_GRPC_SCHEME;

    fn test_writer() -> GcsGrpcWriter {
        let sign_ctx = Context::new();
        let core = Arc::new(GcsGrpcCore {
            info: ServiceInfo::new(GCS_GRPC_SCHEME, "/", "example-bucket"),
            capability: Capability::default(),
            endpoint: "http://127.0.0.1:1".to_string(),
            bucket: "example-bucket".to_string(),
            root: "/".to_string(),
            channel_endpoint: Endpoint::from_static("http://127.0.0.1:1"),
            channel: Default::default(),
            signer: Signer::new(
                sign_ctx.clone(),
                ProvideCredentialChain::new(),
                RequestSigner::new("storage"),
            ),
            sign_ctx,
            skip_signature: true,
        });
        GcsGrpcWriter::new(
            core,
            OperationContext::default(),
            "test",
            OpWrite::default(),
        )
    }

    fn request_content(request: &WriteObjectRequest) -> Option<&Bytes> {
        request.data.as_ref().map(|data| match data {
            write_object_request::Data::ChecksummedData(data) => &data.content,
        })
    }

    #[test]
    fn writer_keeps_the_first_write_on_the_one_shot_path() {
        let mut writer = test_writer();

        block_on(writer.write(Buffer::from(Bytes::from_static(b"hello world")))).unwrap();

        assert!(writer.upload_id.is_none());
        assert!(writer.pending.is_none());
        assert_eq!(
            writer.buffer.unwrap().to_bytes(),
            Bytes::from_static(b"hello world")
        );
    }

    #[test]
    fn write_object_requests_builds_a_finite_put_stream() {
        let body = Buffer::from(vec![
            Bytes::from(vec![1; MAX_WRITE_CHUNK_SIZE - 1]),
            Bytes::from(vec![2; 2]),
            Bytes::from(vec![3; MAX_WRITE_CHUNK_SIZE + 16]),
        ]);
        let requests = block_on(
            write_object_requests(
                body,
                0,
                true,
                write_object_request::FirstMessage::WriteObjectSpec(WriteObjectSpec::default()),
            )
            .unwrap()
            .collect::<Vec<_>>(),
        );

        assert!(matches!(
            requests[0].first_message,
            Some(write_object_request::FirstMessage::WriteObjectSpec(_))
        ));
        assert!(
            requests[1..]
                .iter()
                .all(|request| request.first_message.is_none())
        );
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].write_offset, 0);
        assert_eq!(requests[1].write_offset, MAX_WRITE_CHUNK_SIZE as i64);
        assert_eq!(requests[2].write_offset, (MAX_WRITE_CHUNK_SIZE * 2) as i64);
        assert_eq!(
            request_content(&requests[0]).unwrap().len(),
            MAX_WRITE_CHUNK_SIZE
        );
        assert_eq!(
            request_content(&requests[1]).unwrap().len(),
            MAX_WRITE_CHUNK_SIZE
        );
        assert_eq!(request_content(&requests[2]).unwrap().len(), 17);
        assert!(requests[..2].iter().all(|request| !request.finish_write));
        assert!(requests[2].finish_write);
    }

    #[test]
    fn write_object_requests_finishes_an_empty_object() {
        let requests = block_on(
            write_object_requests(
                Buffer::new(),
                0,
                true,
                write_object_request::FirstMessage::WriteObjectSpec(WriteObjectSpec::default()),
            )
            .unwrap()
            .collect::<Vec<_>>(),
        );

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].write_offset, 0);
        assert!(requests[0].data.is_none());
        assert!(requests[0].finish_write);
        assert!(matches!(
            requests[0].first_message,
            Some(write_object_request::FirstMessage::WriteObjectSpec(_))
        ));
    }

    #[test]
    fn resumable_request_stream_requires_aligned_non_final_data() {
        assert!(
            write_object_requests(
                Buffer::from(vec![0; WRITE_CHUNK_ALIGNMENT - 1]),
                0,
                false,
                write_object_request::FirstMessage::UploadId("upload-id".to_string()),
            )
            .is_err()
        );

        let requests = block_on(
            write_object_requests(
                Buffer::from(vec![0; WRITE_CHUNK_ALIGNMENT]),
                23,
                false,
                write_object_request::FirstMessage::UploadId("upload-id".to_string()),
            )
            .unwrap()
            .collect::<Vec<_>>(),
        );
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].write_offset, 23);
        assert!(!requests[0].finish_write);
        assert!(matches!(
            requests[0].first_message.as_ref(),
            Some(write_object_request::FirstMessage::UploadId(upload_id))
                if upload_id == "upload-id"
        ));
    }

    #[test]
    fn merge_buffers_preserves_fragmented_input() {
        let merged = merge_buffers(
            Buffer::from(vec![Bytes::from_static(b"ab"), Bytes::from_static(b"cd")]),
            Buffer::from(vec![Bytes::from_static(b"ef"), Bytes::from_static(b"gh")]),
        );

        assert_eq!(merged.to_bytes(), Bytes::from_static(b"abcdefgh"));
    }

    #[test]
    fn persisted_size_advances_only_the_pending_range() {
        let mut writer = test_writer();
        writer.pending = Some(PendingUpload {
            body: Buffer::from(vec![1; MAX_WRITE_CHUNK_SIZE * 2]),
            tail: Buffer::from(Bytes::from_static(b"tail")),
            write_offset: 17,
            finish_write: false,
            in_flight: true,
        });

        assert!(
            writer
                .reconcile_persisted_size(17 + MAX_WRITE_CHUNK_SIZE as i64)
                .unwrap()
                .is_none()
        );
        let pending = writer.pending.as_ref().unwrap();
        assert_eq!(pending.write_offset, 17 + MAX_WRITE_CHUNK_SIZE as i64);
        assert_eq!(pending.body.len(), MAX_WRITE_CHUNK_SIZE);
        assert!(!pending.in_flight);

        let outcome = writer
            .reconcile_persisted_size(17 + (MAX_WRITE_CHUNK_SIZE * 2) as i64)
            .unwrap();
        assert!(matches!(
            outcome,
            Some(UploadOutcome::Persisted(size))
                if size == 17 + (MAX_WRITE_CHUNK_SIZE * 2) as i64
        ));
        assert!(writer.pending.as_ref().unwrap().body.is_empty());
    }
}
