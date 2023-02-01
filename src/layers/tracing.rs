// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt::Debug;
use std::io;
use std::io::Read;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;
use bytes::Bytes;
use futures::AsyncRead;
use futures::FutureExt;
use tracing::Span;

use crate::raw::*;
use crate::*;

/// TracingLayer will add tracing for OpenDAL.
///
/// # Examples
///
/// ```
/// use anyhow::Result;
/// use opendal::layers::TracingLayer;
/// use opendal::Operator;
/// use opendal::Scheme;
///
/// let _ = Operator::from_env(Scheme::Fs)
///     .expect("must init")
///     .layer(TracingLayer);
/// ```
pub struct TracingLayer;

impl<A: Accessor> Layer<A> for TracingLayer {
    type LayeredAccessor = TracingAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccessor {
        TracingAccessor { inner }
    }
}

#[derive(Debug)]
struct TracingAccessor<A> {
    inner: A,
}

#[async_trait]
impl<A: Accessor> LayeredAccessor for TracingAccessor<A> {
    type Inner = A;
    type Reader = TracingReader<A::Reader>;
    type BlockingReader = TracingReader<A::BlockingReader>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    #[tracing::instrument(level = "debug")]
    fn metadata(&self) -> AccessorMetadata {
        self.inner.metadata()
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn create(&self, path: &str, args: OpCreate) -> FutureResult<RpCreate> {
        self.inner.create(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn read(&self, path: &str, args: OpRead) -> FutureResult<(RpRead, Self::Reader)> {
        Box::pin(
            self.inner
                .read(path, args)
                .map(|v| v.map(|(rp, r)| (rp, TracingReader::new(Span::current(), r)))),
        )
    }

    #[tracing::instrument(level = "debug", skip(self, r))]
    fn write(&self, path: &str, args: OpWrite, r: input::Reader) -> FutureResult<RpWrite> {
        let r = Box::new(TracingReader::new(Span::current(), r));
        self.inner.write(path, args, r)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn stat(&self, path: &str, args: OpStat) -> FutureResult<RpStat> {
        self.inner.stat(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn delete(&self, path: &str, args: OpDelete) -> FutureResult<RpDelete> {
        self.inner.delete(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn list(&self, path: &str, args: OpList) -> FutureResult<(RpList, ObjectPager)> {
        Box::pin(self.inner.list(path, args).map(|v| {
            v.map(|(rp, s)| {
                (
                    rp,
                    Box::new(TracingPager::new(Span::current(), s)) as ObjectPager,
                )
            })
        }))
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        self.inner.presign(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn create_multipart(
        &self,
        path: &str,
        args: OpCreateMultipart,
    ) -> FutureResult<RpCreateMultipart> {
        self.inner.create_multipart(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self, r))]
    fn write_multipart(
        &self,
        path: &str,
        args: OpWriteMultipart,
        r: input::Reader,
    ) -> FutureResult<RpWriteMultipart> {
        let r = Box::new(TracingReader::new(Span::current(), r));
        self.inner.write_multipart(path, args, r)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn complete_multipart(
        &self,
        path: &str,
        args: OpCompleteMultipart,
    ) -> FutureResult<RpCompleteMultipart> {
        self.inner.complete_multipart(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn abort_multipart(
        &self,
        path: &str,
        args: OpAbortMultipart,
    ) -> FutureResult<RpAbortMultipart> {
        self.inner.abort_multipart(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn blocking_create(&self, path: &str, args: OpCreate) -> Result<RpCreate> {
        self.inner.blocking_create(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        self.inner
            .blocking_read(path, args)
            .map(|(rp, r)| (rp, TracingReader::new(Span::current(), r)))
    }

    #[tracing::instrument(level = "debug", skip(self, r))]
    fn blocking_write(
        &self,
        path: &str,
        args: OpWrite,
        r: input::BlockingReader,
    ) -> Result<RpWrite> {
        self.inner.blocking_write(path, args, r)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.blocking_stat(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn blocking_delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        self.inner.blocking_delete(path, args)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, BlockingObjectPager)> {
        self.inner.blocking_list(path, args).map(|(rp, it)| {
            (
                rp,
                Box::new(BlockingTracingPager::new(Span::current(), it)) as BlockingObjectPager,
            )
        })
    }
}

struct TracingReader<R> {
    span: Span,
    inner: R,
}

impl<R> TracingReader<R> {
    fn new(span: Span, inner: R) -> Self {
        Self { span, inner }
    }
}

impl<R: output::Read> output::Read for TracingReader<R> {
    #[tracing::instrument(
        parent = &self.span,
        level = "trace",
        skip_all)]
    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.inner.poll_read(cx, buf)
    }

    #[tracing::instrument(
        parent = &self.span,
        level = "trace",
        skip_all)]
    fn poll_seek(&mut self, cx: &mut Context<'_>, pos: io::SeekFrom) -> Poll<io::Result<u64>> {
        self.inner.poll_seek(cx, pos)
    }

    #[tracing::instrument(
        parent = &self.span,
        level = "trace",
        skip_all)]
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<io::Result<Bytes>>> {
        self.inner.poll_next(cx)
    }
}

impl<R: input::Read> AsyncRead for TracingReader<R> {
    #[tracing::instrument(
        parent = &self.span,
        level = "trace",
        fields(size = buf.len())
        skip_all)]
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<R: output::BlockingRead> output::BlockingRead for TracingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }

    fn next(&mut self) -> Option<io::Result<Bytes>> {
        self.inner.next()
    }
}

impl<R: input::BlockingRead> Read for TracingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

struct TracingPager {
    span: Span,
    inner: ObjectPager,
}

impl TracingPager {
    fn new(span: Span, streamer: ObjectPager) -> Self {
        Self {
            span,
            inner: streamer,
        }
    }
}

#[async_trait]
impl ObjectPage for TracingPager {
    #[tracing::instrument(parent = &self.span, level = "debug", skip_all)]
    async fn next_page(&mut self) -> Result<Option<Vec<ObjectEntry>>> {
        self.inner.next_page().await
    }
}

struct BlockingTracingPager {
    span: Span,
    inner: BlockingObjectPager,
}

impl BlockingTracingPager {
    fn new(span: Span, inner: BlockingObjectPager) -> Self {
        Self { span, inner }
    }
}

impl BlockingObjectPage for BlockingTracingPager {
    #[tracing::instrument(parent = &self.span, level = "debug", skip_all)]
    fn next_page(&mut self) -> Result<Option<Vec<ObjectEntry>>> {
        self.inner.next_page()
    }
}
