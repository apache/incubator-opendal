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

use await_tree::InstrumentAwait;
use futures::Future;

use crate::raw::*;
use crate::*;

/// Add an Instrument await-tree for actor-based applications to the underlying services.
///
/// # AwaitTree
///
/// await-tree allows developers to dump this execution tree at runtime,
/// with the span of each Future annotated by instrument_await.
/// Read more about [await-tree](https://docs.rs/await-tree/latest/await_tree/)
///
/// # Examples
///
/// ```no_run
/// # use opendal::layers::AwaitTreeLayer;
/// # use opendal::services;
/// # use opendal::Operator;
/// # use opendal::Result;
/// # use opendal::Scheme;
///
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(AwaitTreeLayer::new())
///     .finish();
/// Ok(())
/// # }
/// ```
#[derive(Clone, Default)]
pub struct AwaitTreeLayer {}

impl AwaitTreeLayer {
    /// Create a new `AwaitTreeLayer`.
    pub fn new() -> Self {
        Self {}
    }
}

impl<A: Access> Layer<A> for AwaitTreeLayer {
    type LayeredAccess = AwaitTreeAccessor<A>;

    fn layer(&self, accessor: A) -> Self::LayeredAccess {
        AwaitTreeAccessor { inner: accessor }
    }
}

#[derive(Debug, Clone)]
pub struct AwaitTreeAccessor<A: Access> {
    inner: A,
}

impl<A: Access> LayeredAccess for AwaitTreeAccessor<A> {
    type Inner = A;
    type Reader = AwaitTreeWrapper<A::Reader>;
    type BlockingReader = AwaitTreeWrapper<A::BlockingReader>;
    type Writer = AwaitTreeWrapper<A::Writer>;
    type BlockingWriter = AwaitTreeWrapper<A::BlockingWriter>;
    type Lister = AwaitTreeWrapper<A::Lister>;
    type BlockingLister = AwaitTreeWrapper<A::BlockingLister>;
    type Deleter = AwaitTreeWrapper<A::Deleter>;
    type BlockingDeleter = AwaitTreeWrapper<A::BlockingDeleter>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        self.inner
            .read(path, args)
            .instrument_await(format!("opendal::{}", Operation::Read))
            .await
            .map(|(rp, r)| (rp, AwaitTreeWrapper::new(r)))
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        self.inner
            .write(path, args)
            .instrument_await(format!("opendal::{}", Operation::Write))
            .await
            .map(|(rp, r)| (rp, AwaitTreeWrapper::new(r)))
    }

    async fn copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        self.inner()
            .copy(from, to, args)
            .instrument_await(format!("opendal::{}", Operation::Copy))
            .await
    }

    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.inner()
            .rename(from, to, args)
            .instrument_await(format!("opendal::{}", Operation::Rename))
            .await
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner
            .stat(path, args)
            .instrument_await(format!("opendal::{}", Operation::Stat))
            .await
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        self.inner
            .delete()
            .instrument_await(format!("opendal::{}", Operation::Delete))
            .await
            .map(|(rp, r)| (rp, AwaitTreeWrapper::new(r)))
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        self.inner
            .list(path, args)
            .instrument_await(format!("opendal::{}", Operation::List))
            .await
            .map(|(rp, r)| (rp, AwaitTreeWrapper::new(r)))
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        self.inner
            .presign(path, args)
            .instrument_await(format!("opendal::{}", Operation::Presign))
            .await
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        self.inner
            .blocking_read(path, args)
            .map(|(rp, r)| (rp, AwaitTreeWrapper::new(r)))
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        self.inner
            .blocking_write(path, args)
            .map(|(rp, r)| (rp, AwaitTreeWrapper::new(r)))
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        self.inner
            .blocking_list(path, args)
            .map(|(rp, r)| (rp, AwaitTreeWrapper::new(r)))
    }

    fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
        self.inner
            .blocking_delete()
            .map(|(rp, r)| (rp, AwaitTreeWrapper::new(r)))
    }
}

pub struct AwaitTreeWrapper<R> {
    inner: R,
}

impl<R> AwaitTreeWrapper<R> {
    fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: oio::Read> oio::Read for AwaitTreeWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner
            .read()
            .instrument_await(format!("opendal::{}", Operation::ReaderRead))
            .await
    }
}

impl<R: oio::BlockingRead> oio::BlockingRead for AwaitTreeWrapper<R> {
    fn read(&mut self) -> Result<Buffer> {
        self.inner.read()
    }
}

impl<R: oio::Write> oio::Write for AwaitTreeWrapper<R> {
    fn write(&mut self, bs: Buffer) -> impl Future<Output = Result<()>> + MaybeSend {
        self.inner
            .write(bs)
            .instrument_await(format!("opendal::{}", Operation::WriterWrite.into_static()))
    }

    fn abort(&mut self) -> impl Future<Output = Result<()>> + MaybeSend {
        self.inner
            .abort()
            .instrument_await(format!("opendal::{}", Operation::WriterAbort.into_static()))
    }

    fn close(&mut self) -> impl Future<Output = Result<Metadata>> + MaybeSend {
        self.inner
            .close()
            .instrument_await(format!("opendal::{}", Operation::WriterClose.into_static()))
    }
}

impl<R: oio::BlockingWrite> oio::BlockingWrite for AwaitTreeWrapper<R> {
    fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner.write(bs)
    }

    fn close(&mut self) -> Result<Metadata> {
        self.inner.close()
    }
}

impl<R: oio::List> oio::List for AwaitTreeWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner
            .next()
            .instrument_await(format!("opendal::{}", Operation::ListerNext))
            .await
    }
}

impl<R: oio::BlockingList> oio::BlockingList for AwaitTreeWrapper<R> {
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner.next()
    }
}

impl<R: oio::Delete> oio::Delete for AwaitTreeWrapper<R> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args)
    }

    async fn flush(&mut self) -> Result<usize> {
        self.inner
            .flush()
            .instrument_await(format!("opendal::{}", Operation::DeleterFlush))
            .await
    }
}

impl<R: oio::BlockingDelete> oio::BlockingDelete for AwaitTreeWrapper<R> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args)
    }

    fn flush(&mut self) -> Result<usize> {
        self.inner.flush()
    }
}
