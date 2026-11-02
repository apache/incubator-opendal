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
            .map_err(|_| {
                LfsFetchError::Other(Error::new(
                    ErrorKind::Unexpected,
                    "LFS download request failed",
                ))
            })?;

        let status = response.status();
        if status.is_success() {
            let rp = RpRead::new(MetadataBuilder::file(size).build());
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

impl oio::StreamRead for GitReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let content = self.read_blob().await?;
        if self.core.resolve_lfs() && GitCore::is_lfs_pointer(&content) {
            if let Ok(pointer) = GitCore::parse_lfs_pointer(&content) {
                return self.open_lfs(range, &pointer).await;
            }
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
    use futures::stream;
    use http::{Request, Response, StatusCode};
    use opendal_core::Buffer;
    use opendal_core::HttpBody;
    use opendal_core::HttpTransport;
    use opendal_core::HttpTransporter;
    use opendal_core::Result;
    use opendal_core::raw::oio::ReadStreamDyn as _;
    use sha2::Digest;
    use sha2::Sha256;
    use std::path::Path;
    use std::process::Command;
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
    }

    impl MockTransport {
        fn new(payload: &[u8]) -> Self {
            let oid = format!("{:x}", Sha256::digest(payload));
            Self {
                payload: payload.to_vec(),
                oid,
                authorization: SharedHeader::default(),
                range: SharedHeader::default(),
                batch_count: Arc::new(Mutex::new(0)),
                payload_count: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl HttpTransport for MockTransport {
        async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
            if req.method() == http::Method::POST {
                *self.batch_count.lock().unwrap() += 1;
                let body = serde_json::json!({
                    "objects": [{
                        "oid": self.oid,
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
            Ok(Response::builder()
                .status(StatusCode::OK)
                .body(HttpBody::new(
                    stream::iter(chunks),
                    Some(self.payload.len() as u64),
                ))
                .unwrap())
        }
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git must be installed for the git service tests");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn pointer_repo(payload: &[u8]) -> (tempfile::TempDir, std::path::PathBuf, String, String) {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().to_path_buf();
        git(&dir, &["init", "-q"]);
        git(&dir, &["config", "user.email", "opendal@example.com"]);
        git(&dir, &["config", "user.name", "OpenDAL"]);

        let oid = format!("{:x}", Sha256::digest(payload));
        let pointer = format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
            payload.len()
        );
        std::fs::write(dir.join("model.bin"), pointer).unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "pointer"]);
        let commit = git(&dir, &["rev-parse", "HEAD"]);

        let repository = format!("file://{}", dir.display());
        (temp, dir, repository, commit)
    }

    fn context(transport: MockTransport) -> OperationContext {
        OperationContext::new().with_http_transport(HttpTransporter::new(transport))
    }

    #[tokio::test]
    async fn lfs_reader_streams_action_chunks() {
        let payload = b"0123456789";
        let (_temp, _dir, repository, commit) = pointer_repo(payload);
        let transport = MockTransport::new(payload);
        let core = Arc::new(GitCore::new(
            repository,
            commit,
            "/".to_string(),
            None,
            None,
            true,
        ));
        let reader = GitReader::new(core, context(transport.clone()), "model.bin".to_string());

        let (rp, mut stream) = reader.open(BytesRange::default()).await.unwrap();
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
    async fn lfs_reader_requests_requested_range() {
        let payload = b"0123456789";
        let (_temp, _dir, repository, commit) = pointer_repo(payload);
        let transport = MockTransport::new(payload);
        let core = Arc::new(GitCore::new(
            repository,
            commit,
            "/".to_string(),
            None,
            None,
            true,
        ));
        let reader = GitReader::new(core, context(transport.clone()), "model.bin".to_string());

        let _ = reader.open(BytesRange::new(0, Some(4))).await.unwrap();
        assert_eq!(
            transport.range.lock().unwrap().as_deref(),
            Some("bytes=0-3")
        );
        assert_eq!(*transport.payload_count.lock().unwrap(), 1);
    }
}
