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
use std::sync::Arc;

use opendal_core::raw::*;
use opendal_core::*;
use opentelemetry::Context as TraceContext;
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::trace::FutureExt as TraceFutureExt;
use opentelemetry::trace::Span;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::Tracer;

/// `OtelTraceLayer` traces OpenDAL operations with
/// [OpenTelemetry](https://docs.rs/opentelemetry/latest/opentelemetry/trace/index.html).
///
/// The layer obtains the `opendal` tracer from OpenTelemetry's global tracer
/// provider. Applications must install and configure that provider before
/// issuing operations.
///
/// The layer creates spans for service metadata, create, read, write, copy,
/// rename, stat, list, and presign calls. It carries read, write, and list
/// contexts into their stateful I/O bodies. Delete calls currently pass through
/// without creating a span.
///
/// # Examples
///
/// ## Basic Setup
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_oteltrace::OtelTraceLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(OtelTraceLayer::new());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct OtelTraceLayer {}

impl OtelTraceLayer {
    /// Create a new [`OtelTraceLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for OtelTraceLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl OtelTraceLayer {
    fn layer(&self, inner: Servicer) -> OtelTraceService {
        OtelTraceService { inner }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct OtelTraceService {
    inner: Servicer,
}

impl Service for OtelTraceService {
    type Reader = OtelTraceWrapper<oio::Reader>;
    type Writer = OtelTraceWrapper<oio::Writer>;
    type Lister = OtelTraceWrapper<oio::Lister>;
    type Deleter = oio::Deleter;
    type Copier = oio::Copier;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        let tracer = global::tracer("opendal");
        tracer.in_span("info", |_cx| self.inner.info())
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
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("create");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        self.inner
            .create_dir(ctx, path, args)
            .with_context(cx)
            .await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("read");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        self.inner
            .read(ctx, path, args)
            .map(|r| OtelTraceWrapper::new(cx, r))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("write");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        self.inner
            .write(ctx, path, args)
            .map(|r| OtelTraceWrapper::new(cx, r))
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("copy");
        span.set_attribute(KeyValue::new("from", from.to_string()));
        span.set_attribute(KeyValue::new("to", to.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        let _guard = cx.attach();
        self.inner.copy(ctx, from, to, args)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("rename");
        span.set_attribute(KeyValue::new("from", from.to_string()));
        span.set_attribute(KeyValue::new("to", to.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        self.inner
            .rename(ctx, from, to, args)
            .with_context(cx)
            .await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("restore");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        self.inner.restore(ctx, path, args).with_context(cx).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("stat");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        self.inner.stat(ctx, path, args).with_context(cx).await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner.delete(ctx)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("list");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        self.inner
            .list(ctx, path, args)
            .map(|s| OtelTraceWrapper::new(cx, s))
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("presign");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{args:?}")));
        let cx = TraceContext::current_with_span(span);
        self.inner.presign(ctx, path, args).with_context(cx).await
    }
}

#[doc(hidden)]
pub struct OtelTraceWrapper<R> {
    cx: TraceContext,
    inner: R,
}

impl<R> OtelTraceWrapper<R> {
    fn new(cx: TraceContext, inner: R) -> Self {
        Self { cx, inner }
    }

    fn child_context(&self, name: &'static str, range: BytesRange) -> TraceContext {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start_with_context(name, &self.cx);
        span.set_attribute(KeyValue::new("range", range.to_string()));
        self.cx.with_span(span)
    }
}

impl<R: oio::ReadStream> oio::ReadStream for OtelTraceWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner.read().with_context(self.cx.clone()).await
    }
}

impl<R: oio::Read> oio::Read for OtelTraceWrapper<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let cx = self.child_context("reader.open", range);
        let (rp, stream) = self.inner.open(range).with_context(cx.clone()).await?;
        Ok((
            rp,
            Box::new(OtelTraceWrapper::new(cx, stream)) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let cx = self.child_context("reader.read", range);
        self.inner.read(range).with_context(cx).await
    }
}

impl<R: oio::Write> oio::Write for OtelTraceWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner.write(bs).with_context(self.cx.clone()).await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        self.inner
            .copy_from(path, args, range)
            .with_context(self.cx.clone())
            .await
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner.abort().with_context(self.cx.clone()).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().with_context(self.cx.clone()).await
    }
}

impl<R: oio::List> oio::List for OtelTraceWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner.next().with_context(self.cx.clone()).await
    }
}
