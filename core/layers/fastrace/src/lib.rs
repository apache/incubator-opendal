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
use std::future::Future;
use std::sync::Arc;

use fastrace::prelude::*;
use opendal_core::raw::*;
use opendal_core::*;

/// `FastraceLayer` traces every operation with
/// [fastrace](https://docs.rs/fastrace/).
///
/// It creates spans for service calls and deferred operation bodies
/// such as readers, writers, listers, deleters, and copiers.
///
/// # Examples
///
/// ## Basic Setup
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_fastrace::FastraceLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(FastraceLayer::new());
/// # Ok(())
/// # }
/// ```
///
/// ## Real usage
///
/// ```no_run
/// # use anyhow::Result;
/// # use fastrace::prelude::*;
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_layer_fastrace::FastraceLayer;
/// #
/// # fn main() -> Result<()> {
/// let reporter = fastrace_jaeger::JaegerReporter::new("127.0.0.1:6831".parse()?, "opendal").unwrap();
/// fastrace::set_reporter(reporter, fastrace::collector::Config::default());
///
/// {
///     let root = Span::root("op", SpanContext::random());
///     let runtime = tokio::runtime::Runtime::new()?;
///     runtime.block_on(
///         async {
///             let _ = dotenvy::dotenv();
///             let op = Operator::new(services::Memory::default())?
///                 .layer(FastraceLayer::new());
///             op.write("test", "0".repeat(16 * 1024 * 1024).into_bytes())
///                 .await?;
///             op.stat("test").await?;
///             op.read("test").await?;
///             Ok::<(), opendal_core::Error>(())
///         }
///         .in_span(Span::enter_with_parent("test", &root)),
///     )?;
/// }
///
/// fastrace::flush();
/// # Ok(())
/// # }
/// ```
///
/// # Output
///
/// OpenDAL is using [`fastrace`](https://docs.rs/fastrace/latest/fastrace/) for tracing internally.
///
/// To enable fastrace output, initialize a reporter supported by `fastrace`.
///
/// For example:
///
/// ```no_run
/// # use anyhow::Result;
/// #
/// # fn main() -> Result<()> {
/// let reporter = fastrace_jaeger::JaegerReporter::new("127.0.0.1:6831".parse()?, "opendal").unwrap();
/// fastrace::set_reporter(reporter, fastrace::collector::Config::default());
/// # Ok(())
/// # }
/// ```
///
/// For real-world usage, take a look at [`fastrace-datadog`](https://crates.io/crates/fastrace-datadog) or [`fastrace-jaeger`](https://crates.io/crates/fastrace-jaeger).
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct FastraceLayer {}

impl FastraceLayer {
    /// Create a new [`FastraceLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for FastraceLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl FastraceLayer {
    fn layer(&self, inner: Servicer) -> FastraceAccessor {
        FastraceAccessor { inner }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct FastraceAccessor {
    inner: Servicer,
}

impl Service for FastraceAccessor {
    // Operations with returned bodies continue after the service call returns,
    // so wrap those bodies to trace deferred IO as well.
    type Reader = FastraceWrapper<oio::Reader>;
    type Writer = FastraceWrapper<oio::Writer>;
    type Lister = FastraceWrapper<oio::Lister>;
    type Deleter = FastraceWrapper<oio::Deleter>;
    type Copier = FastraceWrapper<oio::Copier>;
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
        let _guard = Span::enter_with_local_parent(Operation::CreateDir.into_static());
        self.inner.create_dir(ctx, path, args).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let _guard = Span::enter_with_local_parent(Operation::Read.into_static());
        self.inner.read(ctx, path, args).map(|r| {
            FastraceWrapper::new(
                Span::enter_with_local_parent(Operation::Read.into_static()),
                r,
            )
        })
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let _guard = Span::enter_with_local_parent(Operation::Write.into_static());
        self.inner.write(ctx, path, args).map(|r| {
            FastraceWrapper::new(
                Span::enter_with_local_parent(Operation::Write.into_static()),
                r,
            )
        })
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let _guard = Span::enter_with_local_parent(Operation::Copy.into_static());
        self.inner.copy(ctx, from, to, args).map(|c| {
            FastraceWrapper::new(
                Span::enter_with_local_parent(Operation::Copy.into_static()),
                c,
            )
        })
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        let _guard = Span::enter_with_local_parent(Operation::Rename.into_static());
        self.inner.rename(ctx, from, to, args).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        let _guard = Span::enter_with_local_parent(Operation::Restore.into_static());
        self.inner.restore(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let _guard = Span::enter_with_local_parent(Operation::Stat.into_static());
        self.inner.stat(ctx, path, args).await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let _guard = Span::enter_with_local_parent(Operation::Delete.into_static());
        self.inner.delete(ctx).map(|r| {
            FastraceWrapper::new(
                Span::enter_with_local_parent(Operation::Delete.into_static()),
                r,
            )
        })
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let _guard = Span::enter_with_local_parent(Operation::List.into_static());
        self.inner.list(ctx, path, args).map(|s| {
            FastraceWrapper::new(
                Span::enter_with_local_parent(Operation::List.into_static()),
                s,
            )
        })
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let _guard = Span::enter_with_local_parent(Operation::Presign.into_static());
        self.inner.presign(ctx, path, args).await
    }
}

#[doc(hidden)]
// Keep the operation span with the returned body so later body methods can
// attach child spans without relying on local span state.
pub struct FastraceWrapper<R> {
    span: Arc<Span>,
    inner: R,
}

impl<R> FastraceWrapper<R> {
    fn new(span: Span, inner: R) -> Self {
        Self {
            span: Arc::new(span),
            inner,
        }
    }

    fn with_span(span: Arc<Span>, inner: R) -> Self {
        Self { span, inner }
    }
}

impl<R: oio::ReadStream> oio::ReadStream for FastraceWrapper<R> {
    fn read(&mut self) -> impl Future<Output = Result<Buffer>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Read.into_static());
        self.inner.read()
    }
}

impl<R: oio::Read> oio::Read for FastraceWrapper<R> {
    fn open(
        &self,
        range: BytesRange,
    ) -> impl Future<Output = Result<(RpRead, Box<dyn oio::ReadStreamDyn>)>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let span = self.span.clone();
        let fut = self.inner.open(range);
        async move {
            let (rp, stream) = fut.await?;
            Ok((
                rp,
                Box::new(FastraceWrapper::with_span(span, stream)) as Box<dyn oio::ReadStreamDyn>,
            ))
        }
    }

    fn read(
        &self,
        range: BytesRange,
    ) -> impl Future<Output = Result<(RpRead, Buffer)>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Read.into_static());
        self.inner.read(range)
    }
}

impl<R: oio::Write> oio::Write for FastraceWrapper<R> {
    fn write(&mut self, bs: Buffer) -> impl Future<Output = Result<()>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Write.into_static());
        self.inner.write(bs)
    }

    fn copy_from(
        &mut self,
        path: &str,
        args: OpRead,
        range: BytesRange,
    ) -> impl Future<Output = Result<()>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Write.into_static());
        let path = path.to_string();
        async move { self.inner.copy_from(&path, args, range).await }
    }

    fn abort(&mut self) -> impl Future<Output = Result<()>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Write.into_static());
        self.inner.abort()
    }

    fn close(&mut self) -> impl Future<Output = Result<Metadata>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Write.into_static());
        self.inner.close()
    }
}

impl<R: oio::List> oio::List for FastraceWrapper<R> {
    fn next(&mut self) -> impl Future<Output = Result<Option<oio::Entry>>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::List.into_static());
        self.inner.next()
    }
}

impl<R: oio::Delete> oio::Delete for FastraceWrapper<R> {
    fn delete<'a>(
        &'a mut self,
        path: &'a str,
        args: OpDelete,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Delete.into_static());
        self.inner.delete(path, args)
    }

    fn close(&mut self) -> impl Future<Output = Result<()>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Delete.into_static());
        self.inner.close()
    }
}

impl<C: oio::Copy> oio::Copy for FastraceWrapper<C> {
    fn next(&mut self) -> impl Future<Output = Result<Option<usize>>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Copy.into_static());
        self.inner.next()
    }

    fn close(&mut self) -> impl Future<Output = Result<Metadata>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Copy.into_static());
        self.inner.close()
    }

    fn abort(&mut self) -> impl Future<Output = Result<()>> + MaybeSend {
        let _guard = self.span.set_local_parent();
        let _span = LocalSpan::enter_with_local_parent(Operation::Copy.into_static());
        self.inner.abort()
    }
}
