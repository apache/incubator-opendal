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

use std::sync::{Arc, Mutex};
use std::time::Instant;

use bytes::Bytes;
use http::StatusCode;
use http::header;
use opendal_core::raw::*;
use opendal_core::*;

use super::core::GitCore;
use super::core::LfsAction;
use super::core::LfsPointer;
use super::core::join_error;

/// Reader returned by the Git service.
pub struct GitReader {
    core: Arc<GitCore>,
    ctx: OperationContext,
    path: String,
    action: Mutex<Option<LfsAction>>,
}

impl GitReader {
    pub(super) fn new(core: Arc<GitCore>, ctx: OperationContext, path: String) -> Self {
        Self {
            core,
            ctx,
            path,
            action: Mutex::new(None),
        }
    }

    async fn read_blob(&self) -> Result<Bytes> {
        let core = self.core.clone();
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || core.read_blob_blocking(&path))
            .await
            .map_err(join_error)?
    }

    fn cached_action(&self) -> Result<Option<LfsAction>> {
        let guard = self
            .action
            .lock()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "LFS action cache lock poisoned"))?;
        Ok(guard
            .clone()
            .filter(|action| action.is_fresh(Instant::now())))
    }

    fn store_action(&self, action: LfsAction) -> Result<()> {
        let mut guard = self
            .action
            .lock()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "LFS action cache lock poisoned"))?;
        *guard = Some(action);
        Ok(())
    }

    fn invalidate_action(&self) -> Result<()> {
        let mut guard = self
            .action
            .lock()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "LFS action cache lock poisoned"))?;
        *guard = None;
        Ok(())
    }

    async fn open_lfs(
        &self,
        range: BytesRange,
        pointer: &LfsPointer,
    ) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        if range.offset() > pointer.size {
            return Err(Error::new(
                ErrorKind::RangeNotSatisfied,
                "range start offset exceeds content length",
            ));
        }
        if range.offset() == pointer.size || range.size() == Some(0) {
            let rp = RpRead::new(MetadataBuilder::file(pointer.size).build());
            return Ok((rp, Box::new(Bytes::new())));
        }
        if let Some(action) = self.cached_action()? {
            match self.fetch_payload(&action, range, pointer.size).await {
                Ok(result) => return Ok(result),
                Err(LfsFetchError::Rejected(_)) => self.invalidate_action()?,
                Err(LfsFetchError::Other(err)) => return Err(err),
            }
        }

        let action = self
            .core
            .resolve_lfs_action(&self.ctx, &pointer.oid, pointer.size)
            .await?;
        match self.fetch_payload(&action, range, pointer.size).await {
            Ok(result) => {
                self.store_action(action)?;
                Ok(result)
            }
            Err(LfsFetchError::Rejected(err)) | Err(LfsFetchError::Other(err)) => Err(err),
        }
    }

    async fn fetch_payload(
        &self,
        action: &LfsAction,
        range: BytesRange,
        size: u64,
    ) -> std::result::Result<(RpRead, Box<dyn oio::ReadStreamDyn>), LfsFetchError> {
        let mut builder = http::Request::builder()
            .method(http::Method::GET)
            .uri(action.url.as_str());
        for (name, value) in self.core.action_headers(action).iter() {
            if name.as_str() == header::RANGE.as_str() {
                continue;
            }
            builder = builder.header(name.clone(), value.clone());
        }
        if !range.is_full() {
            builder = builder.header(header::RANGE, range.to_header());
        }
        let request = builder.body(Buffer::new()).map_err(|_| {
            LfsFetchError::Other(Error::new(
                ErrorKind::Unexpected,
                "failed to build LFS download request",
            ))
        })?;

        let response = self
            .ctx
            .http_transport()
            .fetch(request)
            .await
            .map_err(LfsFetchError::Other)?;

        let status = response.status();
        if status.is_success() {
            let rp = RpRead::new(MetadataBuilder::file(size).build());
            if status == StatusCode::OK && !range.is_full() {
                return Ok((
                    rp,
                    Box::new(LfsRangeStream {
                        body: response.into_body(),
                        skip: range.offset(),
                        remaining: range
                            .size()
                            .unwrap_or(size - range.offset())
                            .min(size - range.offset()),
                    }),
                ));
            }
            return Ok((
                rp,
                Box::new(response.into_body()) as Box<dyn oio::ReadStreamDyn>,
            ));
        }

        let error = if matches!(
            status,
            StatusCode::UNAUTHORIZED
                | StatusCode::FORBIDDEN
                | StatusCode::NOT_FOUND
                | StatusCode::GONE
        ) {
            LfsFetchError::Rejected(Error::new(
                ErrorKind::PermissionDenied,
                format!("LFS download rejected with status {status}"),
            ))
        } else {
            LfsFetchError::Other(Error::new(
                ErrorKind::Unexpected,
                format!("LFS download failed with status {status}"),
            ))
        };
        Err(error)
    }
}

/// Apply a byte range incrementally when a server sends the full object.
struct LfsRangeStream {
    body: HttpBody,
    skip: u64,
    remaining: u64,
}

impl oio::ReadStream for LfsRangeStream {
    async fn read(&mut self) -> Result<Buffer> {
        while self.remaining > 0 {
            let buffer = self.body.read().await?;
            if buffer.is_empty() {
                return Ok(buffer);
            }
            let length = buffer.len() as u64;
            if self.skip >= length {
                self.skip -= length;
                continue;
            }
            let start = self.skip as usize;
            let length = (length - self.skip).min(self.remaining);
            self.skip = 0;
            self.remaining -= length;
            return Ok(buffer.slice(start..start + length as usize));
        }
        Ok(Buffer::new())
    }
}

impl oio::StreamRead for GitReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let content = self.read_blob().await?;
        if self.core.resolve_lfs()
            && GitCore::is_lfs_pointer(&content)
            && let Ok(pointer) = GitCore::parse_lfs_pointer(&content)
        {
            return self.open_lfs(range, &pointer).await;
        }

        let (rp, body) = GitCore::slice_blob(content, range)?;
        Ok((rp, Box::new(body) as Box<dyn oio::ReadStreamDyn>))
    }
}

enum LfsFetchError {
    Rejected(Error),
    Other(Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::{FutureExt, StreamExt, stream};
    use http::{Request, Response, StatusCode};
    use opendal_core::Buffer;
    use opendal_core::HttpBody;
    use opendal_core::HttpTransport;
    use opendal_core::HttpTransporter;
    use opendal_core::Result;
    use opendal_core::raw::oio::ReadStreamDyn as _;
    use sha2::Digest;
    use sha2::Sha256;
    use std::sync::{Arc, Mutex};

    type SharedHeader = std::sync::Arc<std::sync::Mutex<std::option::Option<std::string::String>>>;

    #[derive(Clone)]
    struct MockTransport {
        payload: Vec<u8>,
        oid: String,
        authorization: SharedHeader,
        range: SharedHeader,
        batch_count: Arc<Mutex<usize>>,
        payload_count: Arc<Mutex<usize>>,
        pending_tail: bool,
        reject_cached: bool,
    }

    impl MockTransport {
        fn new(payload: &[u8]) -> Self {
            let oid = Sha256::digest(payload)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            Self {
                payload: payload.to_vec(),
                oid,
                authorization: SharedHeader::default(),
                range: SharedHeader::default(),
                batch_count: Arc::new(Mutex::new(0)),
                payload_count: Arc::new(Mutex::new(0)),
                pending_tail: false,
                reject_cached: false,
            }
        }
    }

    impl HttpTransport for MockTransport {
        async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
            if req.method() == http::Method::POST {
                *self.batch_count.lock().unwrap() += 1;
                let body = serde_json::json!({
                    "objects": [{
                        "oid": self.oid.clone(),
                        "size": self.payload.len(),
                        "actions": {
                            "download": {
                                "href": "https://cdn.example.net/object?token=action-token",
                                "header": {"Authorization": "Bearer action-token"}
                            }
                        }
                    }]
                });
                let body = Buffer::from(serde_json::to_vec(&body).unwrap());
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .body(HttpBody::new(stream::iter(vec![Ok(body)]), None))
                    .unwrap());
            }

            *self.payload_count.lock().unwrap() += 1;
            if self.reject_cached && *self.payload_count.lock().unwrap() == 2 {
                return Ok(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .body(HttpBody::new(stream::empty(), Some(0)))
                    .unwrap());
            }
            *self.authorization.lock().unwrap() = req
                .headers()
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(ToString::to_string);
            *self.range.lock().unwrap() = req
                .headers()
                .get(http::header::RANGE)
                .and_then(|value| value.to_str().ok())
                .map(ToString::to_string);

            let chunks = vec![
                Ok(Buffer::from(Bytes::copy_from_slice(&self.payload[..4]))),
                Ok(Buffer::from(Bytes::copy_from_slice(&self.payload[4..]))),
            ];
            let body = if self.pending_tail {
                HttpBody::new(
                    stream::iter(chunks.into_iter().take(1)).chain(stream::pending()),
                    Some(self.payload.len() as u64),
                )
            } else {
                HttpBody::new(stream::iter(chunks), Some(self.payload.len() as u64))
            };
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(body)
                .unwrap())
        }
    }

    fn reader(transport: impl HttpTransport) -> GitReader {
        let core = Arc::new(GitCore::new(
            "https://example.com/repo".to_string(),
            "HEAD".to_string(),
            "/".to_string(),
            None,
            None,
            true,
        ));
        let context = OperationContext::new().with_http_transport(HttpTransporter::new(transport));
        GitReader::new(core, context, "model.bin".to_string())
    }

    fn pointer(transport: &MockTransport) -> LfsPointer {
        LfsPointer {
            oid: transport.oid.clone(),
            size: transport.payload.len() as u64,
        }
    }

    #[tokio::test]
    async fn lfs_reader_streams_action_chunks() {
        let payload = b"0123456789";
        let transport = MockTransport::new(payload);
        let reader = reader(transport.clone());

        let (rp, mut stream) = reader
            .open_lfs(BytesRange::default(), &pointer(&transport))
            .await
            .unwrap();
        assert_eq!(rp.metadata().unwrap().content_length(), 10);
        assert_eq!(
            stream.read_dyn().await.unwrap().to_bytes(),
            Bytes::from_static(b"0123")
        );
        assert_eq!(
            stream.read_dyn().await.unwrap().to_bytes(),
            Bytes::from_static(b"456789")
        );
        assert!(stream.read_dyn().await.unwrap().is_empty());
        assert_eq!(
            transport.authorization.lock().unwrap().as_deref(),
            Some("Bearer action-token")
        );
        assert_eq!(*transport.batch_count.lock().unwrap(), 1);
        assert_eq!(*transport.payload_count.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn lfs_reader_slices_ignored_range() {
        let payload = b"0123456789";
        let transport = MockTransport::new(payload);
        let reader = reader(transport.clone());

        let (rp, mut stream) = reader
            .open_lfs(BytesRange::new(2, Some(4)), &pointer(&transport))
            .await
            .unwrap();
        let mut result = Vec::new();
        loop {
            let buffer = stream.read_dyn().await.unwrap();
            if buffer.is_empty() {
                break;
            }
            result.extend_from_slice(&buffer.to_bytes());
        }
        assert_eq!(result, b"2345");
        assert_eq!(rp.metadata().unwrap().content_length(), 10);
        assert_eq!(
            transport.range.lock().unwrap().as_deref(),
            Some("bytes=2-5")
        );
        assert_eq!(*transport.payload_count.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn lfs_range_completes_without_waiting_for_full_body() {
        let mut transport = MockTransport::new(b"0123456789");
        transport.pending_tail = true;
        let reader = reader(transport.clone());
        let pointer = pointer(&transport);
        let (_, mut stream) = reader
            .open_lfs(BytesRange::new(0, Some(4)), &pointer)
            .now_or_never()
            .expect("opening must not buffer the payload")
            .unwrap();
        let first = stream
            .read_dyn()
            .now_or_never()
            .expect("the first chunk is ready")
            .unwrap();
        assert_eq!(first.to_bytes(), Bytes::from_static(b"0123"));
        assert!(
            stream
                .read_dyn()
                .now_or_never()
                .expect("the range is complete despite a pending tail")
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn rejected_cached_action_is_refreshed_once() {
        let mut transport = MockTransport::new(b"0123456789");
        transport.reject_cached = true;
        let reader = reader(transport.clone());
        let pointer = pointer(&transport);
        let _ = reader
            .open_lfs(BytesRange::default(), &pointer)
            .await
            .unwrap();
        let _ = reader
            .open_lfs(BytesRange::default(), &pointer)
            .await
            .unwrap();
        assert_eq!(*transport.batch_count.lock().unwrap(), 2);
        assert_eq!(*transport.payload_count.lock().unwrap(), 3);
    }

    struct TemporaryFailure;

    impl HttpTransport for TemporaryFailure {
        async fn fetch(&self, _: Request<Buffer>) -> Result<Response<HttpBody>> {
            Err(Error::new(ErrorKind::Unexpected, "temporary transport failure").set_temporary())
        }
    }

    #[tokio::test]
    async fn transport_errors_keep_retry_state() {
        let transport = MockTransport::new(b"0123456789");
        let pointer = pointer(&transport);
        let mut reader = reader(transport);
        let action = reader
            .core
            .resolve_lfs_action(&reader.ctx, &pointer.oid, pointer.size)
            .await
            .unwrap();
        reader.ctx =
            OperationContext::new().with_http_transport(HttpTransporter::new(TemporaryFailure));
        match reader
            .fetch_payload(&action, BytesRange::default(), pointer.size)
            .await
        {
            Err(LfsFetchError::Other(error)) => assert!(error.is_temporary()),
            _ => panic!("the transport error must propagate"),
        }
        match reader.open_lfs(BytesRange::default(), &pointer).await {
            Err(error) => assert!(error.is_temporary()),
            _ => panic!("the batch transport error must propagate"),
        }
    }
}
