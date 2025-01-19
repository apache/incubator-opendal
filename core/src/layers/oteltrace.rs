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

use std::future::Future;
use std::sync::Arc;

use opentelemetry::global;
use opentelemetry::global::BoxedSpan;
use opentelemetry::trace::FutureExt as TraceFutureExt;
use opentelemetry::trace::Span;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::Tracer;
use opentelemetry::Context as TraceContext;
use opentelemetry::KeyValue;

use crate::raw::*;
use crate::*;

/// Add [opentelemetry::trace](https://docs.rs/opentelemetry/latest/opentelemetry/trace/index.html) for every operation.
///
/// Examples
///
/// ## Basic Setup
///
/// ```no_run
/// # use opendal::layers::OtelTraceLayer;
/// # use opendal::services;
/// # use opendal::Operator;
/// # use opendal::Result;
///
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(OtelTraceLayer)
///     .finish();
/// Ok(())
/// # }
/// ```
pub struct OtelTraceLayer;

impl<A: Access> Layer<A> for OtelTraceLayer {
    type LayeredAccess = OtelTraceAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        OtelTraceAccessor { inner }
    }
}

#[derive(Debug)]
pub struct OtelTraceAccessor<A> {
    inner: A,
}

impl<A: Access> LayeredAccess for OtelTraceAccessor<A> {
    type Inner = A;
    type Reader = OtelTraceWrapper<A::Reader>;
    type BlockingReader = OtelTraceWrapper<A::BlockingReader>;
    type Writer = OtelTraceWrapper<A::Writer>;
    type BlockingWriter = OtelTraceWrapper<A::BlockingWriter>;
    type Lister = OtelTraceWrapper<A::Lister>;
    type BlockingLister = OtelTraceWrapper<A::BlockingLister>;
    type Deleter = A::Deleter;
    type BlockingDeleter = A::BlockingDeleter;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn info(&self) -> Arc<AccessorInfo> {
        let tracer = global::tracer("opendal");
        tracer.in_span("info", |_cx| self.inner.info())
    }

    async fn create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("create");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        let cx = TraceContext::current_with_span(span);
        self.inner.create_dir(path, args).with_context(cx).await
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("read");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        self.inner
            .read(path, args)
            .await
            .map(|(rp, r)| (rp, OtelTraceWrapper::new(span, r)))
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("write");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        self.inner
            .write(path, args)
            .await
            .map(|(rp, r)| (rp, OtelTraceWrapper::new(span, r)))
    }

    async fn copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("copy");
        span.set_attribute(KeyValue::new("from", from.to_string()));
        span.set_attribute(KeyValue::new("to", to.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        let cx = TraceContext::current_with_span(span);
        self.inner().copy(from, to, args).with_context(cx).await
    }

    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("rename");
        span.set_attribute(KeyValue::new("from", from.to_string()));
        span.set_attribute(KeyValue::new("to", to.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        let cx = TraceContext::current_with_span(span);
        self.inner().rename(from, to, args).with_context(cx).await
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("stat");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        let cx = TraceContext::current_with_span(span);
        self.inner().stat(path, args).with_context(cx).await
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        self.inner().delete().await
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("list");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        self.inner
            .list(path, args)
            .await
            .map(|(rp, s)| (rp, OtelTraceWrapper::new(span, s)))
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("presign");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        let cx = TraceContext::current_with_span(span);
        self.inner().presign(path, args).with_context(cx).await
    }

    fn blocking_create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        let tracer = global::tracer("opendal");
        tracer.in_span("blocking_create_dir", |cx| {
            let span = cx.span(); // let mut span = cx.();
            span.set_attribute(KeyValue::new("path", path.to_string()));
            span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
            self.inner().blocking_create_dir(path, args)
        })
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("blocking_read");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        self.inner
            .blocking_read(path, args)
            .map(|(rp, r)| (rp, OtelTraceWrapper::new(span, r)))
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("blocking_write");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        self.inner
            .blocking_write(path, args)
            .map(|(rp, r)| (rp, OtelTraceWrapper::new(span, r)))
    }

    fn blocking_copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        let tracer = global::tracer("opendal");
        tracer.in_span("blocking_copy", |cx| {
            let span = cx.span();
            span.set_attribute(KeyValue::new("from", from.to_string()));
            span.set_attribute(KeyValue::new("to", to.to_string()));
            span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
            self.inner().blocking_copy(from, to, args)
        })
    }

    fn blocking_rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        let tracer = global::tracer("opendal");
        tracer.in_span("blocking_rename", |cx| {
            let span = cx.span();
            span.set_attribute(KeyValue::new("from", from.to_string()));
            span.set_attribute(KeyValue::new("to", to.to_string()));
            span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
            self.inner().blocking_rename(from, to, args)
        })
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let tracer = global::tracer("opendal");
        tracer.in_span("blocking_stat", |cx| {
            let span = cx.span();
            span.set_attribute(KeyValue::new("path", path.to_string()));
            span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
            self.inner().blocking_stat(path, args)
        })
    }

    fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
        self.inner().blocking_delete()
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        let tracer = global::tracer("opendal");
        let mut span = tracer.start("blocking_list");
        span.set_attribute(KeyValue::new("path", path.to_string()));
        span.set_attribute(KeyValue::new("args", format!("{:?}", args)));
        self.inner
            .blocking_list(path, args)
            .map(|(rp, it)| (rp, OtelTraceWrapper::new(span, it)))
    }
}

pub struct OtelTraceWrapper<R> {
    _span: BoxedSpan,
    inner: R,
}

impl<R> OtelTraceWrapper<R> {
    fn new(_span: BoxedSpan, inner: R) -> Self {
        Self { _span, inner }
    }
}

impl<R: oio::Read> oio::Read for OtelTraceWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner.read().await
    }
}

impl<R: oio::BlockingRead> oio::BlockingRead for OtelTraceWrapper<R> {
    fn read(&mut self) -> Result<Buffer> {
        self.inner.read()
    }
}

impl<R: oio::Write> oio::Write for OtelTraceWrapper<R> {
    fn write(&mut self, bs: Buffer) -> impl Future<Output = Result<()>> + MaybeSend {
        self.inner.write(bs)
    }

    fn abort(&mut self) -> impl Future<Output = Result<()>> + MaybeSend {
        self.inner.abort()
    }

    fn close(&mut self) -> impl Future<Output = Result<Metadata>> + MaybeSend {
        self.inner.close()
    }
}

impl<R: oio::BlockingWrite> oio::BlockingWrite for OtelTraceWrapper<R> {
    fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner.write(bs)
    }

    fn close(&mut self) -> Result<Metadata> {
        self.inner.close()
    }
}

impl<R: oio::List> oio::List for OtelTraceWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner.next().await
    }
}

impl<R: oio::BlockingList> oio::BlockingList for OtelTraceWrapper<R> {
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner.next()
    }
}
