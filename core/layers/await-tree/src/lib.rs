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
use await_tree::InstrumentAwait;
use std::sync::Arc;

use opendal_core::raw::*;
use opendal_core::*;

/// `AwaitTreeLayer` instruments service operations for actor-based applications.
///
/// # AwaitTree
///
/// `await-tree` lets developers dump the execution tree at runtime and annotates
/// each future's span with `instrument_await`.
/// Read more about [await-tree](https://docs.rs/await-tree/latest/await_tree/)
///
/// # Examples
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_await_tree::AwaitTreeLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(AwaitTreeLayer::new());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct AwaitTreeLayer {}

impl AwaitTreeLayer {
    /// Create a new [`AwaitTreeLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for AwaitTreeLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl AwaitTreeLayer {
    fn layer(&self, inner: Servicer) -> AwaitTreeAccessor {
        AwaitTreeAccessor { inner }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct AwaitTreeAccessor {
    inner: Servicer,
}

impl Service for AwaitTreeAccessor {
    type Reader = AwaitTreeWrapper<oio::Reader>;
    type Writer = AwaitTreeWrapper<oio::Writer>;
    type Lister = AwaitTreeWrapper<oio::Lister>;
    type Deleter = AwaitTreeWrapper<oio::Deleter>;
    type Copier = AwaitTreeWrapper<oio::Copier>;
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
        self.inner
            .create_dir(ctx, path, args)
            .instrument_await(format!("opendal::{}", Operation::CreateDir))
            .await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        self.inner.read(ctx, path, args).map(AwaitTreeWrapper::new)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.inner.write(ctx, path, args).map(AwaitTreeWrapper::new)
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
            .map(AwaitTreeWrapper::new)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.inner
            .rename(ctx, from, to, args)
            .instrument_await(format!("opendal::{}", Operation::Rename))
            .await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.inner
            .restore(ctx, path, args)
            .instrument_await(format!("opendal::{}", Operation::Restore))
            .await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner
            .stat(ctx, path, args)
            .instrument_await(format!("opendal::{}", Operation::Stat))
            .await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner.delete(ctx).map(AwaitTreeWrapper::new)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.inner.list(ctx, path, args).map(AwaitTreeWrapper::new)
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.inner
            .presign(ctx, path, args)
            .instrument_await(format!("opendal::{}", Operation::Presign))
            .await
    }
}

#[doc(hidden)]
pub struct AwaitTreeWrapper<R> {
    inner: R,
}

impl<R> AwaitTreeWrapper<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: oio::ReadStream> oio::ReadStream for AwaitTreeWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner
            .read()
            .instrument_await(format!("opendal::{}", Operation::Read))
            .await
    }
}

impl<R: oio::Read> oio::Read for AwaitTreeWrapper<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let (rp, stream) = self
            .inner
            .open(range)
            .instrument_await(format!("opendal::{}", Operation::Read))
            .await?;
        Ok((
            rp,
            Box::new(AwaitTreeWrapper::new(stream)) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        self.inner
            .read(range)
            .instrument_await(format!("opendal::{}", Operation::Read))
            .await
    }
}

impl<R: oio::Write> oio::Write for AwaitTreeWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner
            .write(bs)
            .instrument_await(format!("opendal::{}", Operation::Write.into_static()))
            .await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        self.inner
            .copy_from(path, args, range)
            .instrument_await(format!("opendal::{}", Operation::Write.into_static()))
            .await
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner
            .abort()
            .instrument_await(format!("opendal::{}", Operation::Write.into_static()))
            .await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner
            .close()
            .instrument_await(format!("opendal::{}", Operation::Write.into_static()))
            .await
    }
}

impl<R: oio::List> oio::List for AwaitTreeWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner
            .next()
            .instrument_await(format!("opendal::{}", Operation::List))
            .await
    }
}

impl<R: oio::Delete> oio::Delete for AwaitTreeWrapper<R> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args).await
    }

    async fn close(&mut self) -> Result<()> {
        self.inner
            .close()
            .instrument_await(format!("opendal::{}", Operation::Delete))
            .await
    }
}

impl<C: oio::Copy> oio::Copy for AwaitTreeWrapper<C> {
    async fn next(&mut self) -> Result<Option<usize>> {
        self.inner
            .next()
            .instrument_await(format!("opendal::{}", Operation::Copy.into_static()))
            .await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner
            .close()
            .instrument_await(format!("opendal::{}", Operation::Copy.into_static()))
            .await
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner
            .abort()
            .instrument_await(format!("opendal::{}", Operation::Copy.into_static()))
            .await
    }
}
