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

use super::backend::*;
use super::core::HfDownloadMode;
use super::core::{HfCore, HfReadResponse, XetFileResponse};
use asyncband::once::OnceCell;
use http::Response;
use opendal_core::raw::*;
use opendal_core::*;
use std::ops::Range;
use xet::xet_session::{SessionError, XetDownloadStream, XetDownloadStreamGroup, XetFileInfo};

pub enum HfReadStream {
    Http(HttpBody),
    Xet(XetDownloadStream),
}

/// Converts an opendal byte range into the `Option<Range<u64>>` the `xet`
/// crate expects: `None` for a full read, `Some(start..end)` otherwise, with
/// an open-ended range (`size` unknown) mapped to `start..u64::MAX`.
fn xet_range(range: BytesRange) -> Option<Range<u64>> {
    if range.is_full() {
        None
    } else {
        let start = range.offset();
        let end = range.size().map(|s| start + s).unwrap_or(u64::MAX);
        Some(start..end)
    }
}

impl HfReadStream {
    /// Build the stream directly from an already-known XET hash + size,
    /// resolved or reused by [`HfCore`].
    async fn new_xet(
        group: &XetDownloadStreamGroup,
        hash: &str,
        size: u64,
        range: BytesRange,
    ) -> Result<(RpRead, Self)> {
        let metadata = {
            let metadata = MetadataBuilder::file(size);
            metadata.build()
        };
        let xet_range = xet_range(range);

        let mut stream = group
            .download_stream(XetFileInfo::new(hash.to_string(), size), xet_range)
            .await
            .map_err(|err| {
                Error::new(
                    ErrorKind::Unexpected,
                    "failed to create xet download stream",
                )
                .set_source(err)
            })?;
        stream.start();
        Ok((RpRead::new(metadata), Self::Xet(stream)))
    }

    /// Build the stream from a plain (non-XET) resolve response: it already
    /// carries the range-correct bytes, so there is nothing left to fetch.
    fn new_http(path: &str, resp: Response<HttpBody>) -> Result<(RpRead, Self)> {
        let metadata = parse_into_metadata(path, resp.headers())?;
        Ok((RpRead::new(metadata), Self::Http(resp.into_body())))
    }
}

fn map_session_error(e: SessionError) -> Error {
    Error::new(ErrorKind::Unexpected, "xet read error").set_source(e)
}

impl oio::ReadStream for HfReadStream {
    async fn read(&mut self) -> Result<Buffer> {
        match self {
            Self::Http(body) => body.read().await,
            Self::Xet(stream) => match stream.next().await {
                Ok(Some(bytes)) => Ok(Buffer::from(bytes)),
                Ok(None) => Ok(Buffer::new()),
                Err(e) => Err(map_session_error(e)),
            },
        }
    }
}

/// Reader returned by this backend.
pub struct HfReader {
    backend: HfBackend,
    ctx: OperationContext,
    path: String,
    // Keep one XET file version for this reader, independently of the backend's
    // optional cross-reader cache. Concurrent ranges share initialization;
    // failed resolutions remain retryable. None means the first read used HTTP.
    resolved: OnceCell<Option<XetFileResponse>>,
    // Built once per reader and reused for every later XET range, seeded
    // from `HfCore`'s cached CAS token. Reuse across concurrent ranges
    // relies on the `xet` crate's documented (not type-enforced) support
    // for many streams per group.
    xet_group: OnceCell<XetDownloadStreamGroup>,
}

impl HfReader {
    pub(super) fn new(backend: HfBackend, ctx: OperationContext, path: &str, _: OpRead) -> Self {
        Self {
            backend,
            ctx,
            path: path.to_string(),
            resolved: OnceCell::new(),
            xet_group: OnceCell::new(),
        }
    }

    /// Build a XET stream via this reader's cached group, creating it on
    /// first use.
    async fn xet_stream(
        &self,
        core: &HfCore,
        hash: &str,
        size: u64,
        range: BytesRange,
    ) -> Result<(RpRead, HfReadStream)> {
        let group = self
            .xet_group
            .get_or_try_init(|| core.xet_download_group(&self.ctx))
            .await?;
        HfReadStream::new_xet(group, hash, size, range).await
    }
}

impl oio::StreamRead for HfReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let core = &self.backend.core;
        let path = self.path.as_str();

        let mut first_http_response = None;
        let resolved = if core.download_mode == HfDownloadMode::Xet {
            self.resolved
                .get_or_try_init(async || {
                    match core.read(&self.ctx, path, range).await? {
                        HfReadResponse::Xet(info) => Ok(Some(info)),
                        HfReadResponse::Http(resp) => {
                            // This response already contains the initializing caller's
                            // range. Keep its bytes instead of probing and discarding them.
                            first_http_response = Some(resp);
                            Ok(None)
                        }
                    }
                })
                .await?
                .as_ref()
        } else {
            None
        };
        let response = match resolved {
            Some(info) => HfReadResponse::Xet(info.clone()),
            None => match first_http_response {
                Some(resp) => HfReadResponse::Http(resp),
                None => core.read(&self.ctx, path, range).await?,
            },
        };
        let (rp, stream) = match response {
            HfReadResponse::Xet(info) => {
                self.xet_stream(core, &info.hash, info.size, range).await?
            }
            HfReadResponse::Http(resp) => HfReadStream::new_http(path, resp)?,
        };

        Ok((rp, Box::new(stream) as Box<dyn oio::ReadStreamDyn>))
    }
}

#[cfg(test)]
mod tests {
    use super::super::backend::test_utils::{
        mbpp_operator, miscased_mbpp_operator, testing_dataset_core,
    };
    use super::super::core::HfRepoType;
    use super::super::core::test_utils::create_test_core;
    use super::super::core::{CommitFile, DeletedFile, HfCore};
    use super::*;
    use bytes::Bytes;
    use opendal_core::raw::oio::{ReadStream, StreamRead};
    use std::sync::Arc;

    /// Parquet magic bytes: "PAR1"
    const PARQUET_MAGIC: &[u8] = b"PAR1";

    fn hf_reader(core: HfCore, ctx: OperationContext, path: &str) -> HfReader {
        hf_reader_from_arc(Arc::new(core), ctx, path)
    }

    fn hf_reader_from_arc(core: Arc<HfCore>, ctx: OperationContext, path: &str) -> HfReader {
        let backend = HfBackend { core };
        HfReader::new(backend, ctx, path, OpRead::default())
    }

    #[test]
    fn test_xet_range_conversion() {
        assert_eq!(xet_range(BytesRange::default()), None);
        assert_eq!(xet_range(BytesRange::new(4, Some(4))), Some(4..8));
        assert_eq!(xet_range(BytesRange::new(4, None)), Some(4..u64::MAX));
    }

    #[tokio::test]
    async fn test_default_reads_resolve_each_range_across_readers() -> Result<()> {
        for mode in [HfDownloadMode::Http, HfDownloadMode::Xet] {
            let (mut core, ctx, mock_client) = create_test_core(
                HfRepoType::Model,
                "test-user/test-repo",
                "main",
                "https://huggingface.co",
            );
            core.download_mode = mode;
            let core = Arc::new(core);
            let first = hf_reader_from_arc(core.clone(), ctx.clone(), "plain.txt");
            let second = hf_reader_from_arc(core, ctx, "plain.txt");

            for (index, reader) in [&first, &first, &second].into_iter().enumerate() {
                let (_, mut stream) = reader.open(BytesRange::new(index as u64, Some(1))).await?;
                assert!(!stream.read().await?.is_empty());
                assert_eq!(mock_client.request_count(), index + 1);
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_http_read_uses_resolve_url() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        let reader = hf_reader(core, ctx, "config.json");

        let (_, mut stream) = reader.open(BytesRange::default()).await?;

        assert_eq!(
            mock_client.get_captured_url(),
            "https://huggingface.co/test-user/test-repo/resolve/main/config.json"
        );
        let chunk = stream.read().await?;
        assert_eq!(chunk.to_bytes(), Bytes::from_static(b"hello"));

        Ok(())
    }

    #[tokio::test]
    async fn test_http_read_returns_metadata() -> Result<()> {
        let (core, ctx, _) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        let reader = hf_reader(core, ctx, "test.txt");

        let (rp, mut stream) = reader.open(BytesRange::default()).await?;
        let metadata = rp.metadata().expect("read metadata must be returned");

        assert_eq!(metadata.mode(), EntryMode::FILE);
        assert_eq!(metadata.content_length(), 5);

        let chunk = stream.read().await?;
        assert_eq!(chunk.to_bytes(), Bytes::from_static(b"hello"));

        Ok(())
    }

    /// Classification (`None` for a non-XET file) is cached, but a non-XET
    /// file has no separate metadata step to skip for its actual bytes: the
    /// classifying resolve's body is discarded and every open, including the
    /// first, fetches its own bytes with its own resolve. So the first open
    /// costs 2 (classify + fetch) and every later one costs 1 (fetch only,
    /// classification already cached).
    #[tokio::test]
    async fn test_non_xet_reads_cache_classification_but_still_fetch_each_range() -> Result<()> {
        let (mut core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        core.enable_resolve_cache = true;
        let reader = hf_reader(core, ctx, "plain.txt");

        let (_, mut s1) = reader.open(BytesRange::new(0, Some(1))).await?;
        s1.read().await?;
        assert_eq!(mock_client.request_count(), 2);

        let (_, mut s2) = reader.open(BytesRange::new(1, Some(1))).await?;
        s2.read().await?;
        assert_eq!(mock_client.request_count(), 3);

        Ok(())
    }

    /// A failed classifying resolve must not permanently cache a failure:
    /// the next `open()` on the same reader should retry rather than being
    /// stuck erroring for the reader's whole lifetime.
    #[tokio::test]
    async fn test_classification_retries_after_resolve_failure() -> Result<()> {
        let (mut core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        core.enable_resolve_cache = true;
        let reader = hf_reader(core, ctx, "plain.txt");
        mock_client.fail_next_requests(1);

        let result = reader.open(BytesRange::new(0, Some(1))).await;
        assert!(
            result.is_err(),
            "a failed classifying resolve must surface as an error"
        );
        assert_eq!(mock_client.request_count(), 1);

        let (_, mut stream) = reader.open(BytesRange::new(0, Some(1))).await?;
        stream.read().await?;
        assert_eq!(
            mock_client.request_count(),
            3,
            "classification must retry (1 resolve) then fetch the range (1 more)"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_xet_ranges_reuse_reader_resolution() -> Result<()> {
        for enabled in [false, true] {
            let (mut core, ctx, mock_client) = create_test_core(
                HfRepoType::Model,
                "test-user/test-repo",
                "main",
                "https://huggingface.co",
            );
            core.enable_resolve_cache = enabled;
            mock_client.set_xet_backed(&"00".repeat(32), 64);
            let core = Arc::new(core);
            let reader = hf_reader_from_arc(core.clone(), ctx.clone(), "xet-file.bin");

            let (r1, r2, r3) = futures::join!(
                reader.open(BytesRange::new(0, Some(1))),
                reader.open(BytesRange::new(1, Some(1))),
                reader.open(BytesRange::new(2, Some(1))),
            );
            r1?;
            r2?;
            r3?;
            // One resolve and one CAS token request; actual CAS bytes are
            // covered by the live multiple-range test below.
            assert_eq!(mock_client.request_count(), 2);

            mock_client.set_xet_backed(&"11".repeat(32), 128);
            let (rp, _) = reader.open(BytesRange::new(3, Some(1))).await?;
            assert_eq!(rp.metadata().unwrap().content_length(), 64);
            assert_eq!(
                reader.resolved.get().unwrap().as_ref().unwrap().hash,
                "00".repeat(32)
            );
            assert_eq!(mock_client.request_count(), 2);

            let fresh = hf_reader_from_arc(core, ctx, "xet-file.bin");
            let (rp, _) = fresh.open(BytesRange::new(0, Some(1))).await?;
            assert_eq!(
                rp.metadata().unwrap().content_length(),
                if enabled { 64 } else { 128 }
            );
            assert_eq!(mock_client.request_count(), if enabled { 2 } else { 3 });
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_resolution_retries_after_failure() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        mock_client.set_xet_backed(&"00".repeat(32), 64);
        mock_client.fail_next_requests(1);
        let reader = hf_reader(core, ctx, "xet-file.bin");
        assert!(reader.open(BytesRange::new(0, Some(1))).await.is_err());
        assert!(reader.resolved.get().is_none());
        reader.open(BytesRange::new(0, Some(1))).await?;
        reader.open(BytesRange::new(1, Some(1))).await?;
        assert_eq!(mock_client.request_count(), 3);
        Ok(())
    }

    /// The probe asks for a fixed single byte, never the caller's range.
    /// Verified against the live API: XET metadata is byte-identical for no
    /// range, a head range, and a range far into the file, so one response
    /// serves every caller -- and for a path that turns out not to be
    /// XET-backed the discarded body is one byte instead of the whole file
    /// (248,987 B measured before this).
    #[tokio::test]
    async fn test_xet_probe_uses_fixed_single_byte_range() -> Result<()> {
        let (mut core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        core.enable_resolve_cache = true;
        mock_client.set_xet_backed(&"00".repeat(32), 64);
        mock_client.set_xet_token_expires_at(u64::MAX);
        let reader = hf_reader(core, ctx, "xet-file.bin");

        reader.open(BytesRange::new(8, Some(4))).await?;

        assert_eq!(
            mock_client.get_captured_classify_range_header().as_deref(),
            Some("bytes=0-0"),
            "the probe must not inherit the caller's range"
        );

        Ok(())
    }

    /// Http mode never classifies. HF puts `x-xet-hash` on the 302 it
    /// answers with, not on the CDN response the transport follows it to, so
    /// a response that does carry the header is still streamed as bytes
    /// rather than parsed as metadata -- and no separate probe is issued.
    #[tokio::test]
    async fn test_http_mode_streams_bytes_without_probing() -> Result<()> {
        let (mut core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        core.download_mode = HfDownloadMode::Http;
        mock_client.set_xet_backed(&"11".repeat(32), 64);
        let reader = hf_reader(core, ctx, "file.bin");

        let (_, mut stream) = reader.open(BytesRange::new(0, Some(4))).await?;
        let chunk = stream.read().await?;

        assert!(
            chunk.to_bytes().starts_with(br#"{"hash""#),
            "the body must be handed back verbatim, not parsed"
        );
        assert_eq!(mock_client.request_count(), 1, "one resolve, no probe");

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_non_xet_ranges_keep_initial_response() -> Result<()> {
        for enabled in [false, true] {
            let (mut core, ctx, mock_client) = create_test_core(
                HfRepoType::Model,
                "test-user/test-repo",
                "main",
                "https://huggingface.co",
            );
            core.enable_resolve_cache = enabled;
            let reader = hf_reader(core, ctx, "plain.txt");

            let (r1, r2, r3) = futures::join!(
                reader.open(BytesRange::new(0, Some(1))),
                reader.open(BytesRange::new(1, Some(1))),
                reader.open(BytesRange::new(2, Some(1))),
            );
            for result in [r1, r2, r3] {
                assert_eq!(result?.1.read().await?.to_vec(), b"hello");
            }
            // Each range fetches its bytes once. Only the backend cache's
            // optional classification adds a one-byte probe.
            assert_eq!(mock_client.request_count(), if enabled { 4 } else { 3 });
        }
        Ok(())
    }

    /// Regression test for #8107 against the live API: a repo id whose case
    /// differs from the canonical one.
    ///
    /// `stat` is the operation that matters here. It posts to `paths-info`,
    /// and HF answers a miscased id with a `307` carrying a path-only
    /// `Location`. A transport will follow that for a `GET` but hands a
    /// bodied `POST` back unfollowed, which is exactly the failure #8107
    /// reported, so this only passes if `HfCore::send` re-issues the request.
    /// The read then covers the `resolve` path through the same repo. Both
    /// use a public dataset, so no token is needed.
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_miscased_repo_id_follows_redirects() {
        let path = "full/train-00000-of-00001.parquet";
        let op = miscased_mbpp_operator();

        let meta = op
            .stat(path)
            .await
            .expect("stat must follow the case redirect on its paths-info POST");
        assert!(meta.content_length() > 0);

        let bytes = op
            .read_with(path)
            .range(0..4)
            .await
            .expect("read must succeed against a miscased repo id")
            .to_vec();
        assert_eq!(bytes, PARQUET_MAGIC);
    }

    /// Exercises the XET download code path against a public dataset known to
    /// have XET-stored files. Behavior tests cannot reliably cover this path
    /// because the test dataset may not contain any XET files.
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_read_xet_parquet() {
        let op = mbpp_operator();
        let data = op
            .read("full/train-00000-of-00001.parquet")
            .await
            .expect("xet read should succeed");
        let bytes = data.to_vec();
        assert!(bytes.len() > 8);
        assert_eq!(&bytes[..4], PARQUET_MAGIC);
        assert_eq!(&bytes[bytes.len() - 4..], PARQUET_MAGIC);
    }

    /// Verifies that a non-XET file (plain git blob) read in Xet mode falls back
    /// to the HTTP body path rather than erroring. Uploads a small file via the
    /// git commit API (which does not go through XET), then reads it back.
    /// Requires HF_OPENDAL_DATASET and HF_OPENDAL_TOKEN.
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_xet_mode_falls_back_to_http_for_non_xet_file() {
        use base64::Engine;

        let core = testing_dataset_core();
        let ctx = OperationContext::new().with_http_transport(HttpTransporter::new(
            opendal_http_transport_reqwest::ReqwestTransport::default(),
        ));
        let content = b"non-xet fallback test content";
        let path = "tests/non-xet-fallback.txt";

        core.commit_git(
            &ctx,
            vec![CommitFile {
                path: path.to_string(),
                content: base64::prelude::BASE64_STANDARD.encode(content),
                encoding: "base64".to_string(),
            }],
            vec![],
            vec![],
            vec![],
        )
        .await
        .expect("commit should succeed");

        let reader = hf_reader_from_arc(core, ctx, path);
        let (_, mut stream) = reader
            .open(BytesRange::default())
            .await
            .expect("reading non-XET file in Xet mode should succeed via HTTP fallback");

        let mut buf = Vec::new();
        loop {
            let chunk: Buffer = stream.read().await.expect("read chunk should succeed");
            if chunk.is_empty() {
                break;
            }
            buf.extend_from_slice(&chunk.to_bytes());
        }
        assert_eq!(buf, content);

        let core = &reader.backend.core;
        core.commit_git(
            &reader.ctx,
            vec![],
            vec![],
            vec![DeletedFile {
                path: path.to_string(),
            }],
            vec![],
        )
        .await
        .ok();
    }

    /// Exercises XET range reads (XetDownloadStream with a byte range).
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_read_xet_range() {
        let op = mbpp_operator();
        let data = op
            .read_with("full/train-00000-of-00001.parquet")
            .range(0..4)
            .await
            .expect("xet range read should succeed");
        let bytes = data.to_vec();
        assert_eq!(bytes.len(), 4);
        assert_eq!(&bytes, PARQUET_MAGIC);
    }

    /// Read separated ranges through one Reader against a public XET file.
    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_read_xet_multiple_ranges_on_one_reader() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Debug, Clone, Default)]
        struct CountingTransport {
            inner: opendal_http_transport_reqwest::ReqwestTransport,
            resolves: Arc<AtomicUsize>,
        }

        impl HttpTransport for CountingTransport {
            async fn fetch(&self, req: http::Request<Buffer>) -> Result<Response<HttpBody>> {
                if req.uri().path().contains("/resolve/") {
                    self.resolves.fetch_add(1, Ordering::SeqCst);
                }
                self.inner.fetch(req).await
            }
        }

        let transport = CountingTransport::default();
        let op = Operator::new(
            super::super::backend::HfBuilder::default()
                .repo_type("dataset")
                .repo_id("google-research-datasets/mbpp")
                .download_mode("xet"),
        )
        .unwrap()
        .with_context(
            OperationContext::new().with_http_transport(HttpTransporter::new(transport.clone())),
        );
        let path = "full/train-00000-of-00001.parquet";
        let reader = op.reader_with(path).concurrent(3).gap(0).await.unwrap();
        let ranges = vec![0..4, 16..20, 32..36];
        let bufs = reader.fetch(ranges.clone()).await.unwrap();
        assert_eq!(bufs[0].to_vec(), PARQUET_MAGIC);
        assert!(bufs.iter().all(|buf| buf.len() == 4));
        let first_resolves = transport.resolves.load(Ordering::SeqCst);
        let repeated = reader.fetch(ranges.clone()).await.unwrap();
        for (first, second) in bufs.iter().zip(repeated) {
            assert_eq!(first.to_vec(), second.to_vec());
        }
        let repeated_resolves = transport.resolves.load(Ordering::SeqCst);
        let fresh = op.read_with(path).range(0..36).await.unwrap().to_vec();
        for (range, buf) in ranges.iter().zip(&bufs) {
            assert_eq!(
                buf.to_vec(),
                fresh[range.start as usize..range.end as usize]
            );
        }
        let fresh_resolves = transport.resolves.load(Ordering::SeqCst);
        eprintln!(
            "resolve counts: first batch={first_resolves}, repeated batch={repeated_resolves}, fresh reader={fresh_resolves}"
        );
        assert_eq!(first_resolves, 1);
        assert_eq!(repeated_resolves, 1);
        assert_eq!(fresh_resolves, 2);
    }
}
