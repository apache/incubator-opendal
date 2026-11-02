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

/// `AsyncBacktraceLayer` records efficient logical stack traces for asynchronous
/// service operations.
///
/// # Async Backtrace
///
/// `async-backtrace` lets developers inspect the stack traces of asynchronous functions.
/// Read more about [async-backtrace](https://docs.rs/async-backtrace/latest/async_backtrace/)
///
/// # Examples
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_async_backtrace::AsyncBacktraceLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(AsyncBacktraceLayer::new());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct AsyncBacktraceLayer {}

impl AsyncBacktraceLayer {
    /// Create a new [`AsyncBacktraceLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for AsyncBacktraceLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl AsyncBacktraceLayer {
    fn layer(&self, inner: Servicer) -> AsyncBacktraceAccessor {
        AsyncBacktraceAccessor { inner }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct AsyncBacktraceAccessor {
    inner: Servicer,
}

impl Service for AsyncBacktraceAccessor {
    type Reader = AsyncBacktraceWrapper<oio::Reader>;
    type Writer = AsyncBacktraceWrapper<oio::Writer>;
    type Lister = AsyncBacktraceWrapper<oio::Lister>;
    type Deleter = AsyncBacktraceWrapper<oio::Deleter>;
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

    #[async_backtrace::framed]
    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.inner.create_dir(ctx, path, args).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        self.inner
            .read(ctx, path, args)
            .map(AsyncBacktraceWrapper::new)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.inner
            .write(ctx, path, args)
            .map(AsyncBacktraceWrapper::new)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        self.inner.copy(ctx, from, to, args)
    }

    #[async_backtrace::framed]
    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.inner.rename(ctx, from, to, args).await
    }

    #[async_backtrace::framed]
    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.inner.restore(ctx, path, args).await
    }

    #[async_backtrace::framed]
    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(ctx, path, args).await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner.delete(ctx).map(AsyncBacktraceWrapper::new)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.inner
            .list(ctx, path, args)
            .map(AsyncBacktraceWrapper::new)
    }

    #[async_backtrace::framed]
    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.inner.presign(ctx, path, args).await
    }
}

#[doc(hidden)]
pub struct AsyncBacktraceWrapper<R> {
    inner: R,
}

impl<R> AsyncBacktraceWrapper<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: oio::ReadStream> oio::ReadStream for AsyncBacktraceWrapper<R> {
    #[async_backtrace::framed]
    async fn read(&mut self) -> Result<Buffer> {
        self.inner.read().await
    }
}

impl<R: oio::Read> oio::Read for AsyncBacktraceWrapper<R> {
    #[async_backtrace::framed]
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let (rp, stream) = self.inner.open(range).await?;
        Ok((
            rp,
            Box::new(AsyncBacktraceWrapper::new(stream)) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    #[async_backtrace::framed]
    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        self.inner.read(range).await
    }
}

impl<R: oio::Write> oio::Write for AsyncBacktraceWrapper<R> {
    #[async_backtrace::framed]
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner.write(bs).await
    }

    #[async_backtrace::framed]
    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        self.inner.copy_from(path, args, range).await
    }

    #[async_backtrace::framed]
    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().await
    }

    #[async_backtrace::framed]
    async fn abort(&mut self) -> Result<()> {
        self.inner.abort().await
    }
}

impl<R: oio::List> oio::List for AsyncBacktraceWrapper<R> {
    #[async_backtrace::framed]
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner.next().await
    }
}

impl<R: oio::Delete> oio::Delete for AsyncBacktraceWrapper<R> {
    #[async_backtrace::framed]
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args).await
    }

    #[async_backtrace::framed]
    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }
}
