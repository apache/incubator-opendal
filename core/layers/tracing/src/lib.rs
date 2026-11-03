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
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use futures::Stream;
use futures::StreamExt;
use opendal_core::raw::*;
use opendal_core::*;
use tracing::Instrument;
use tracing::Level;
use tracing::Span;
use tracing::span;

/// `TracingLayer` traces every operation with
/// [tracing](https://docs.rs/tracing/).
///
/// # Examples
///
/// ## Basic Setup
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_tracing::TracingLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(TracingLayer::new());
/// # Ok(())
/// # }
/// ```
///
/// ## Real usage
///
/// ```no_run
/// # use anyhow::Result;
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_layer_tracing::TracingLayer;
/// # use tracing_subscriber::prelude::*;
/// # use tracing_subscriber::EnvFilter;
/// #
/// # fn main() -> Result<()> {
/// let opentelemetry = tracing_opentelemetry::layer();
///
/// tracing_subscriber::registry()
///     .with(EnvFilter::from_default_env())
///     .with(opentelemetry)
///     .try_init()?;
///
/// {
///     let runtime = tokio::runtime::Runtime::new()?;
///     runtime.block_on(async {
///         let root = tracing::span!(tracing::Level::INFO, "app_start", work_units = 2);
///         let _enter = root.enter();
///
///         let _ = dotenvy::dotenv();
///         let op = Operator::new(services::Memory::default())?
///             .layer(TracingLayer::new());
///
///         op.write("test", "0".repeat(16 * 1024 * 1024).into_bytes())
///             .await?;
///         op.stat("test").await?;
///         op.read("test").await?;
///         Ok::<(), opendal_core::Error>(())
///     })?;
/// }
///
/// # Ok(())
/// # }
/// ```
///
/// # Output
///
/// OpenDAL is using [`tracing`](https://docs.rs/tracing/latest/tracing/) for tracing internally.
///
/// To enable tracing output, please init one of the subscribers that `tracing` supports.
///
/// For example:
///
/// ```no_run
/// # use tracing::dispatcher;
/// # use tracing::Event;
/// # use tracing::Metadata;
/// # use tracing::span::Attributes;
/// # use tracing::span::Id;
/// # use tracing::span::Record;
/// # use tracing::subscriber::Subscriber;
/// #
/// # pub struct FooSubscriber;
/// # impl Subscriber for FooSubscriber {
/// #   fn enabled(&self, _: &Metadata) -> bool { false }
/// #   fn new_span(&self, _: &Attributes) -> Id { Id::from_u64(0) }
/// #   fn record(&self, _: &Id, _: &Record) {}
/// #   fn record_follows_from(&self, _: &Id, _: &Id) {}
/// #   fn event(&self, _: &Event) {}
/// #   fn enter(&self, _: &Id) {}
/// #   fn exit(&self, _: &Id) {}
/// # }
/// # impl FooSubscriber { fn new() -> Self { FooSubscriber } }
///
/// let my_subscriber = FooSubscriber::new();
/// tracing::subscriber::set_global_default(my_subscriber).expect("setting tracing default failed");
/// ```
///
/// For real-world usage, please take a look at [`tracing-opentelemetry`](https://crates.io/crates/tracing-opentelemetry).
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct TracingLayer {}

impl TracingLayer {
    /// Create a new [`TracingLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for TracingLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }

    fn apply_context(&self, _srv: Servicer, inner: OperationContext) -> OperationContext {
        // Give outbound HTTP requests and their response bodies dedicated spans.
        let transport = HttpTransporter::new(TracingHttpTransport {
            inner: inner.http_transport().clone(),
        });
        let executor = Executor::with(TracingExecutor {
            inner: inner.executor().clone().into_inner(),
        });

        inner.with_http_transport(transport).with_executor(executor)
    }
}

impl TracingLayer {
    fn layer(&self, inner: Servicer) -> TracingService {
        TracingService { inner }
    }
}

struct TracingHttpTransport {
    inner: HttpTransporter,
}

impl HttpTransport for TracingHttpTransport {
    async fn fetch(&self, req: http::Request<Buffer>) -> Result<http::Response<HttpBody>> {
        let span = span!(Level::DEBUG, "http::fetch", ?req);

        let resp = self.inner.fetch(req).instrument(span.clone()).await?;

        let (parts, body) = resp.into_parts();
        // Keep response body polling inside the same HTTP fetch span.
        let body = body.map_inner(|s| Box::new(TracingStream { inner: s, span }));
        Ok(http::Response::from_parts(parts, body))
    }
}

struct TracingExecutor {
    inner: Arc<dyn Execute>,
}

impl Execute for TracingExecutor {
    fn execute(&self, f: BoxedStaticFuture<()>) {
        self.inner
            .execute(Box::pin(f.instrument(Span::current())) as BoxedStaticFuture<()>)
    }

    fn timeout(&self) -> Option<BoxedStaticFuture<()>> {
        self.inner.timeout()
    }
}

struct TracingStream<S> {
    inner: S,
    span: Span,
}

impl<S> Stream for TracingStream<S>
where
    S: Stream<Item = Result<Buffer>> + Unpin + 'static,
{
    type Item = Result<Buffer>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let _enter = self.span.clone().entered();
        self.inner.poll_next_unpin(cx)
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct TracingService {
    inner: Servicer,
}

impl Service for TracingService {
    type Reader = TracingWrapper<oio::Reader>;
    type Writer = TracingWrapper<oio::Writer>;
    type Lister = TracingWrapper<oio::Lister>;
    type Deleter = TracingWrapper<oio::Deleter>;
    type Copier = oio::Copier;
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
        let span = span!(Level::DEBUG, "create_dir", path, ?args);
        self.inner
            .create_dir(ctx, path, args)
            .instrument(span)
            .await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let span = span!(Level::DEBUG, "read", path, ?args);
        self.inner
            .read(ctx, path, args)
            .map(|r| TracingWrapper::new(span, r))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let span = span!(Level::DEBUG, "write", path, ?args);
        self.inner
            .write(ctx, path, args)
            .map(|r| TracingWrapper::new(span, r))
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let span = span!(Level::DEBUG, "copy", from, to, ?args);
        let _guard = span.enter();
        self.inner.copy(ctx, from, to, args)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        let span = span!(Level::DEBUG, "rename", from, to, ?args);
        self.inner
            .rename(ctx, from, to, args)
            .instrument(span)
            .await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        let span = span!(Level::DEBUG, "restore", path, ?args);
        self.inner.restore(ctx, path, args).instrument(span).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let span = span!(Level::DEBUG, "stat", path, ?args);
        self.inner.stat(ctx, path, args).instrument(span).await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let span = span!(Level::DEBUG, "delete");
        self.inner.delete(ctx).map(|r| TracingWrapper::new(span, r))
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let span = span!(Level::DEBUG, "list", path, ?args);
        self.inner
            .list(ctx, path, args)
            .map(|r| TracingWrapper::new(span, r))
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let span = span!(Level::DEBUG, "presign", path, ?args);
        self.inner.presign(ctx, path, args).instrument(span).await
    }
}

#[doc(hidden)]
pub struct TracingWrapper<R> {
    span: Span,
    inner: R,
}

impl<R> TracingWrapper<R> {
    fn new(span: Span, inner: R) -> Self {
        Self { span, inner }
    }
}

impl<R: oio::ReadStream> oio::ReadStream for TracingWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner.read().instrument(self.span.clone()).await
    }
}

impl<R: oio::Read> oio::Read for TracingWrapper<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let span = span!(parent: &self.span, Level::DEBUG, "reader.open", range = %range);
        let (rp, stream) = self.inner.open(range).instrument(span.clone()).await?;
        Ok((
            rp,
            Box::new(TracingWrapper::new(span, stream)) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let span = span!(parent: &self.span, Level::DEBUG, "reader.read", range = %range);
        self.inner.read(range).instrument(span).await
    }
}

impl<R: oio::Write> oio::Write for TracingWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner.write(bs).instrument(self.span.clone()).await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        self.inner
            .copy_from(path, args, range)
            .instrument(self.span.clone())
            .await
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner.abort().instrument(self.span.clone()).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().instrument(self.span.clone()).await
    }
}

impl<R: oio::List> oio::List for TracingWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner.next().instrument(self.span.clone()).await
    }
}

impl<R: oio::Delete> oio::Delete for TracingWrapper<R> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner
            .delete(path, args)
            .instrument(self.span.clone())
            .await
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.close().instrument(self.span.clone()).await
    }
}
