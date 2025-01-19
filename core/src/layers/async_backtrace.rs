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

use crate::raw::*;
use crate::*;

/// Add Efficient, logical 'stack' traces of async functions for the underlying services.
///
/// # Async Backtrace
///
/// async-backtrace allows developers to get a stack trace of the async functions.
/// Read more about [async-backtrace](https://docs.rs/async-backtrace/latest/async_backtrace/)
///
/// # Examples
///
/// ```no_run
/// # use opendal::layers::AsyncBacktraceLayer;
/// # use opendal::services;
/// # use opendal::Operator;
/// # use opendal::Result;
/// # use opendal::Scheme;
///
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(AsyncBacktraceLayer::default())
///     .finish();
/// Ok(())
/// # }
/// ```
#[derive(Clone, Default)]
pub struct AsyncBacktraceLayer;

impl<A: Access> Layer<A> for AsyncBacktraceLayer {
    type LayeredAccess = AsyncBacktraceAccessor<A>;

    fn layer(&self, accessor: A) -> Self::LayeredAccess {
        AsyncBacktraceAccessor { inner: accessor }
    }
}

#[derive(Debug, Clone)]
pub struct AsyncBacktraceAccessor<A: Access> {
    inner: A,
}

impl<A: Access> LayeredAccess for AsyncBacktraceAccessor<A> {
    type Inner = A;
    type Reader = AsyncBacktraceWrapper<A::Reader>;
    type BlockingReader = AsyncBacktraceWrapper<A::BlockingReader>;
    type Writer = AsyncBacktraceWrapper<A::Writer>;
    type BlockingWriter = AsyncBacktraceWrapper<A::BlockingWriter>;
    type Lister = AsyncBacktraceWrapper<A::Lister>;
    type BlockingLister = AsyncBacktraceWrapper<A::BlockingLister>;
    type Deleter = AsyncBacktraceWrapper<A::Deleter>;
    type BlockingDeleter = AsyncBacktraceWrapper<A::BlockingDeleter>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    #[async_backtrace::framed]
    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        self.inner
            .read(path, args)
            .await
            .map(|(rp, r)| (rp, AsyncBacktraceWrapper::new(r)))
    }

    #[async_backtrace::framed]
    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        self.inner
            .write(path, args)
            .await
            .map(|(rp, r)| (rp, AsyncBacktraceWrapper::new(r)))
    }

    #[async_backtrace::framed]
    async fn copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        self.inner.copy(from, to, args).await
    }

    #[async_backtrace::framed]
    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.inner.rename(from, to, args).await
    }

    #[async_backtrace::framed]
    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(path, args).await
    }

    #[async_backtrace::framed]
    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        self.inner
            .delete()
            .await
            .map(|(rp, r)| (rp, AsyncBacktraceWrapper::new(r)))
    }

    #[async_backtrace::framed]
    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        self.inner
            .list(path, args)
            .await
            .map(|(rp, r)| (rp, AsyncBacktraceWrapper::new(r)))
    }

    #[async_backtrace::framed]
    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        self.inner.presign(path, args).await
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        self.inner
            .blocking_read(path, args)
            .map(|(rp, r)| (rp, AsyncBacktraceWrapper::new(r)))
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        self.inner
            .blocking_write(path, args)
            .map(|(rp, r)| (rp, AsyncBacktraceWrapper::new(r)))
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        self.inner
            .blocking_list(path, args)
            .map(|(rp, r)| (rp, AsyncBacktraceWrapper::new(r)))
    }

    fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
        self.inner
            .blocking_delete()
            .map(|(rp, r)| (rp, AsyncBacktraceWrapper::new(r)))
    }
}

pub struct AsyncBacktraceWrapper<R> {
    inner: R,
}

impl<R> AsyncBacktraceWrapper<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: oio::Read> oio::Read for AsyncBacktraceWrapper<R> {
    #[async_backtrace::framed]
    async fn read(&mut self) -> Result<Buffer> {
        self.inner.read().await
    }
}

impl<R: oio::BlockingRead> oio::BlockingRead for AsyncBacktraceWrapper<R> {
    fn read(&mut self) -> Result<Buffer> {
        self.inner.read()
    }
}

impl<R: oio::Write> oio::Write for AsyncBacktraceWrapper<R> {
    #[async_backtrace::framed]
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner.write(bs).await
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

impl<R: oio::BlockingWrite> oio::BlockingWrite for AsyncBacktraceWrapper<R> {
    fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner.write(bs)
    }

    fn close(&mut self) -> Result<Metadata> {
        self.inner.close()
    }
}

impl<R: oio::List> oio::List for AsyncBacktraceWrapper<R> {
    #[async_backtrace::framed]
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner.next().await
    }
}

impl<R: oio::BlockingList> oio::BlockingList for AsyncBacktraceWrapper<R> {
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner.next()
    }
}

impl<R: oio::Delete> oio::Delete for AsyncBacktraceWrapper<R> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args)
    }

    #[async_backtrace::framed]
    async fn flush(&mut self) -> Result<usize> {
        self.inner.flush().await
    }
}

impl<R: oio::BlockingDelete> oio::BlockingDelete for AsyncBacktraceWrapper<R> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args)
    }

    fn flush(&mut self) -> Result<usize> {
        self.inner.flush()
    }
}
