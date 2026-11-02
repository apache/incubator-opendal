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

#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use futures::Stream;
use futures::StreamExt;
use opendal_core::raw::*;
use opendal_core::*;

const LABEL_CREATE_DIR: &str = "opendal.create_dir";
const LABEL_READ: &str = "opendal.read";
const LABEL_RENAME: &str = "opendal.rename";
const LABEL_RESTORE: &str = "opendal.restore";
const LABEL_STAT: &str = "opendal.stat";
const LABEL_PRESIGN: &str = "opendal.presign";

const LABEL_READER_READ: &str = "opendal.reader.read";
const LABEL_WRITER_WRITE: &str = "opendal.writer.write";
const LABEL_WRITER_CLOSE: &str = "opendal.writer.close";
const LABEL_WRITER_ABORT: &str = "opendal.writer.abort";
const LABEL_LISTER_NEXT: &str = "opendal.lister.next";
const LABEL_DELETER_DELETE: &str = "opendal.deleter.delete";
const LABEL_DELETER_CLOSE: &str = "opendal.deleter.close";
const LABEL_COPIER_NEXT: &str = "opendal.copier.next";
const LABEL_COPIER_CLOSE: &str = "opendal.copier.close";
const LABEL_COPIER_ABORT: &str = "opendal.copier.abort";
const LABEL_HTTP_FETCH: &str = "opendal.http.fetch";
const LABEL_HTTP_BODY_POLL: &str = "opendal.http.body.poll";

/// `HotpathLayer` profiles every operation with
/// [hotpath](https://docs.rs/hotpath/).
///
/// # Notes
///
/// When `hotpath` profiling is enabled, initialize a guard via
/// [`hotpath::HotpathGuardBuilder`] or `#[hotpath::main]` before running
/// operations. Otherwise, hotpath will panic on the first measurement.
///
/// # Examples
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_hotpath::HotpathLayer;
/// #
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let _guard = hotpath::HotpathGuardBuilder::new("opendal").build();
/// let op = Operator::new(services::Memory::default())?
///     .layer(HotpathLayer::new());
/// op.write("test", "hello").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct HotpathLayer {}

impl HotpathLayer {
    /// Create a new [`HotpathLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for HotpathLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }

    fn apply_context(&self, _srv: Servicer, inner: OperationContext) -> OperationContext {
        let transport = HttpTransporter::new(HotpathHttpTransport {
            inner: inner.http_transport().clone(),
        });
        inner.with_http_transport(transport)
    }
}

impl HotpathLayer {
    fn layer(&self, inner: Servicer) -> HotpathAccessor {
        HotpathAccessor { inner }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct HotpathAccessor {
    inner: Servicer,
}

impl Service for HotpathAccessor {
    type Reader = HotpathWrapper<oio::Reader>;
    type Writer = HotpathWrapper<oio::Writer>;
    type Lister = HotpathWrapper<oio::Lister>;
    type Deleter = HotpathWrapper<oio::Deleter>;
    type Copier = HotpathWrapper<oio::Copier>;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.inner.info()
    }

    fn capability(&self) -> Capability {
        self.inner.capability()
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.inner.compose(ctx, to, args)
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        hotpath::measure_async(LABEL_CREATE_DIR, self.inner.create_dir(ctx, path, args)).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        self.inner.read(ctx, path, args).map(HotpathWrapper::new)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.inner.write(ctx, path, args).map(HotpathWrapper::new)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        self.inner
            .copy(ctx, from, to, args)
            .map(HotpathWrapper::new)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        hotpath::measure_async(LABEL_RENAME, self.inner.rename(ctx, from, to, args)).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        hotpath::measure_async(LABEL_RESTORE, self.inner.restore(ctx, path, args)).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        hotpath::measure_async(LABEL_STAT, self.inner.stat(ctx, path, args)).await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner.delete(ctx).map(HotpathWrapper::new)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.inner.list(ctx, path, args).map(HotpathWrapper::new)
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        hotpath::measure_async(LABEL_PRESIGN, self.inner.presign(ctx, path, args)).await
    }
}

#[doc(hidden)]
pub struct HotpathWrapper<R> {
    inner: R,
}

impl<R> HotpathWrapper<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: oio::ReadStream> oio::ReadStream for HotpathWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        hotpath::measure_async(LABEL_READER_READ, self.inner.read()).await
    }
}

impl<R: oio::Read> oio::Read for HotpathWrapper<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let (rp, stream) = hotpath::measure_async(LABEL_READ, self.inner.open(range)).await?;
        Ok((
            rp,
            Box::new(HotpathWrapper::new(stream)) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        hotpath::measure_async(LABEL_READER_READ, self.inner.read(range)).await
    }
}

impl<R: oio::Write> oio::Write for HotpathWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        hotpath::measure_async(LABEL_WRITER_WRITE, self.inner.write(bs)).await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        hotpath::measure_async(LABEL_WRITER_WRITE, self.inner.copy_from(path, args, range)).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        hotpath::measure_async(LABEL_WRITER_CLOSE, self.inner.close()).await
    }

    async fn abort(&mut self) -> Result<()> {
        hotpath::measure_async(LABEL_WRITER_ABORT, self.inner.abort()).await
    }
}

impl<R: oio::List> oio::List for HotpathWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        hotpath::measure_async(LABEL_LISTER_NEXT, self.inner.next()).await
    }
}

impl<R: oio::Delete> oio::Delete for HotpathWrapper<R> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        hotpath::measure_async(LABEL_DELETER_DELETE, self.inner.delete(path, args)).await
    }

    async fn close(&mut self) -> Result<()> {
        hotpath::measure_async(LABEL_DELETER_CLOSE, self.inner.close()).await
    }
}

impl<C: oio::Copy> oio::Copy for HotpathWrapper<C> {
    async fn next(&mut self) -> Result<Option<usize>> {
        hotpath::measure_async(LABEL_COPIER_NEXT, self.inner.next()).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        hotpath::measure_async(LABEL_COPIER_CLOSE, self.inner.close()).await
    }

    async fn abort(&mut self) -> Result<()> {
        hotpath::measure_async(LABEL_COPIER_ABORT, self.inner.abort()).await
    }
}

struct HotpathHttpTransport {
    inner: HttpTransporter,
}

impl HttpTransport for HotpathHttpTransport {
    async fn fetch(&self, req: http::Request<Buffer>) -> Result<http::Response<HttpBody>> {
        let resp = hotpath::measure_async(LABEL_HTTP_FETCH, self.inner.fetch(req)).await?;
        let (parts, body) = resp.into_parts();
        let body = body.map_inner(|stream| Box::new(HotpathStream { inner: stream }));
        Ok(http::Response::from_parts(parts, body))
    }
}

struct HotpathStream<S> {
    inner: S,
}

impl<S> Stream for HotpathStream<S>
where
    S: Stream<Item = Result<Buffer>> + Unpin + 'static,
{
    type Item = Result<Buffer>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let _label = LABEL_HTTP_BODY_POLL;
        hotpath::measure_block!(_label, self.inner.poll_next_unpin(cx))
    }
}
