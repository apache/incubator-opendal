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

use std::fmt::Debug;
use std::fmt::Formatter;
use std::sync::Arc;

use crate::raw::*;
use crate::*;

/// ErrorContextLayer will add error context into all layers.
///
/// # Notes
///
/// This layer will add the following error context into all errors:
///
/// - `service`: The [`Scheme`] of underlying service.
/// - `operation`: The [`Operation`] of this operation
/// - `path`: The path of this operation
///
/// Some operations may have additional context:
///
/// - `range`: The range the read operation is trying to read.
/// - `read`: The already read size in given reader.
/// - `size`: The size of the current write operation.
/// - `written`: The already written size in given writer.
/// - `listed`: The already listed size in given lister.
/// - `deleted`: The already deleted size in given deleter.
pub struct ErrorContextLayer;

impl<A: Access> Layer<A> for ErrorContextLayer {
    type LayeredAccess = ErrorContextAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        let info = inner.info();
        ErrorContextAccessor { info, inner }
    }
}

/// Provide error context wrapper for backend.
pub struct ErrorContextAccessor<A: Access> {
    info: Arc<AccessorInfo>,
    inner: A,
}

impl<A: Access> Debug for ErrorContextAccessor<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<A: Access> LayeredAccess for ErrorContextAccessor<A> {
    type Inner = A;
    type Reader = ErrorContextWrapper<A::Reader>;
    type BlockingReader = ErrorContextWrapper<A::BlockingReader>;
    type Writer = ErrorContextWrapper<A::Writer>;
    type BlockingWriter = ErrorContextWrapper<A::BlockingWriter>;
    type Lister = ErrorContextWrapper<A::Lister>;
    type BlockingLister = ErrorContextWrapper<A::BlockingLister>;
    type Deleter = ErrorContextWrapper<A::Deleter>;
    type BlockingDeleter = ErrorContextWrapper<A::BlockingDeleter>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn info(&self) -> Arc<AccessorInfo> {
        self.info.clone()
    }

    async fn create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        self.inner.create_dir(path, args).await.map_err(|err| {
            err.with_operation(Operation::CreateDir)
                .with_context("service", self.info.scheme())
                .with_context("path", path)
        })
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let range = args.range();
        self.inner
            .read(path, args)
            .await
            .map(|(rp, r)| {
                (
                    rp,
                    ErrorContextWrapper::new(self.info.scheme(), path.to_string(), r)
                        .with_range(range),
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::Read)
                    .with_context("service", self.info.scheme())
                    .with_context("path", path)
                    .with_context("range", range.to_string())
            })
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        self.inner
            .write(path, args)
            .await
            .map(|(rp, w)| {
                (
                    rp,
                    ErrorContextWrapper::new(self.info.scheme(), path.to_string(), w),
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::Write)
                    .with_context("service", self.info.scheme())
                    .with_context("path", path)
            })
    }

    async fn copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        self.inner.copy(from, to, args).await.map_err(|err| {
            err.with_operation(Operation::Copy)
                .with_context("service", self.info.scheme())
                .with_context("from", from)
                .with_context("to", to)
        })
    }

    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.inner.rename(from, to, args).await.map_err(|err| {
            err.with_operation(Operation::Rename)
                .with_context("service", self.info.scheme())
                .with_context("from", from)
                .with_context("to", to)
        })
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(path, args).await.map_err(|err| {
            err.with_operation(Operation::Stat)
                .with_context("service", self.info.scheme())
                .with_context("path", path)
        })
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        self.inner
            .delete()
            .await
            .map(|(rp, w)| {
                (
                    rp,
                    ErrorContextWrapper::new(self.info.scheme(), "".to_string(), w),
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::Delete)
                    .with_context("service", self.info.scheme())
            })
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        self.inner
            .list(path, args)
            .await
            .map(|(rp, p)| {
                (
                    rp,
                    ErrorContextWrapper::new(self.info.scheme(), path.to_string(), p),
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::List)
                    .with_context("service", self.info.scheme())
                    .with_context("path", path)
            })
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        self.inner.presign(path, args).await.map_err(|err| {
            err.with_operation(Operation::Presign)
                .with_context("service", self.info.scheme())
                .with_context("path", path)
        })
    }

    fn blocking_create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        self.inner.blocking_create_dir(path, args).map_err(|err| {
            err.with_operation(Operation::BlockingCreateDir)
                .with_context("service", self.info.scheme())
                .with_context("path", path)
        })
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        let range = args.range();
        self.inner
            .blocking_read(path, args)
            .map(|(rp, os)| {
                (
                    rp,
                    ErrorContextWrapper::new(self.info.scheme(), path.to_string(), os)
                        .with_range(range),
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::BlockingRead)
                    .with_context("service", self.info.scheme())
                    .with_context("path", path)
                    .with_context("range", range.to_string())
            })
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        self.inner
            .blocking_write(path, args)
            .map(|(rp, os)| {
                (
                    rp,
                    ErrorContextWrapper::new(self.info.scheme(), path.to_string(), os),
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::BlockingWrite)
                    .with_context("service", self.info.scheme())
                    .with_context("path", path)
            })
    }

    fn blocking_copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        self.inner.blocking_copy(from, to, args).map_err(|err| {
            err.with_operation(Operation::BlockingCopy)
                .with_context("service", self.info.scheme())
                .with_context("from", from)
                .with_context("to", to)
        })
    }

    fn blocking_rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.inner.blocking_rename(from, to, args).map_err(|err| {
            err.with_operation(Operation::BlockingRename)
                .with_context("service", self.info.scheme())
                .with_context("from", from)
                .with_context("to", to)
        })
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.blocking_stat(path, args).map_err(|err| {
            err.with_operation(Operation::BlockingStat)
                .with_context("service", self.info.scheme())
                .with_context("path", path)
        })
    }

    fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
        self.inner
            .blocking_delete()
            .map(|(rp, w)| {
                (
                    rp,
                    ErrorContextWrapper::new(self.info.scheme(), "".to_string(), w),
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::BlockingDelete)
                    .with_context("service", self.info.scheme())
            })
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        self.inner
            .blocking_list(path, args)
            .map(|(rp, os)| {
                (
                    rp,
                    ErrorContextWrapper::new(self.info.scheme(), path.to_string(), os),
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::BlockingList)
                    .with_context("service", self.info.scheme())
                    .with_context("path", path)
            })
    }
}

pub struct ErrorContextWrapper<T> {
    scheme: Scheme,
    path: String,
    inner: T,
    range: BytesRange,
    processed: u64,
}

impl<T> ErrorContextWrapper<T> {
    fn new(scheme: Scheme, path: String, inner: T) -> Self {
        Self {
            scheme,
            path,
            inner,
            range: BytesRange::default(),
            processed: 0,
        }
    }

    fn with_range(mut self, range: BytesRange) -> Self {
        self.range = range;
        self
    }
}

impl<T: oio::Read> oio::Read for ErrorContextWrapper<T> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner
            .read()
            .await
            .map(|bs| {
                self.processed += bs.len() as u64;
                bs
            })
            .map_err(|err| {
                err.with_operation(Operation::ReaderRead)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("range", self.range.to_string())
                    .with_context("read", self.processed.to_string())
            })
    }
}

impl<T: oio::BlockingRead> oio::BlockingRead for ErrorContextWrapper<T> {
    fn read(&mut self) -> Result<Buffer> {
        self.inner
            .read()
            .map(|bs| {
                self.processed += bs.len() as u64;
                bs
            })
            .map_err(|err| {
                err.with_operation(Operation::BlockingReaderRead)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("range", self.range.to_string())
                    .with_context("read", self.processed.to_string())
            })
    }
}

impl<T: oio::Write> oio::Write for ErrorContextWrapper<T> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let size = bs.len();
        self.inner
            .write(bs)
            .await
            .map(|_| {
                self.processed += size as u64;
            })
            .map_err(|err| {
                err.with_operation(Operation::WriterWrite)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("size", size.to_string())
                    .with_context("written", self.processed.to_string())
            })
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().await.map_err(|err| {
            err.with_operation(Operation::WriterClose)
                .with_context("service", self.scheme)
                .with_context("path", &self.path)
                .with_context("written", self.processed.to_string())
        })
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner.abort().await.map_err(|err| {
            err.with_operation(Operation::WriterAbort)
                .with_context("service", self.scheme)
                .with_context("path", &self.path)
                .with_context("processed", self.processed.to_string())
        })
    }
}

impl<T: oio::BlockingWrite> oio::BlockingWrite for ErrorContextWrapper<T> {
    fn write(&mut self, bs: Buffer) -> Result<()> {
        let size = bs.len();
        self.inner
            .write(bs)
            .map(|_| {
                self.processed += size as u64;
            })
            .map_err(|err| {
                err.with_operation(Operation::BlockingWriterWrite)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("size", size.to_string())
                    .with_context("written", self.processed.to_string())
            })
    }

    fn close(&mut self) -> Result<Metadata> {
        self.inner.close().map_err(|err| {
            err.with_operation(Operation::BlockingWriterClose)
                .with_context("service", self.scheme)
                .with_context("path", &self.path)
                .with_context("written", self.processed.to_string())
        })
    }
}

impl<T: oio::List> oio::List for ErrorContextWrapper<T> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner
            .next()
            .await
            .map(|bs| {
                self.processed += bs.is_some() as u64;
                bs
            })
            .map_err(|err| {
                err.with_operation(Operation::ListerNext)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("listed", self.processed.to_string())
            })
    }
}

impl<T: oio::BlockingList> oio::BlockingList for ErrorContextWrapper<T> {
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner
            .next()
            .map(|bs| {
                self.processed += bs.is_some() as u64;
                bs
            })
            .map_err(|err| {
                err.with_operation(Operation::BlockingListerNext)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("listed", self.processed.to_string())
            })
    }
}

impl<T: oio::Delete> oio::Delete for ErrorContextWrapper<T> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args).map_err(|err| {
            err.with_operation(Operation::DeleterDelete)
                .with_context("service", self.scheme)
                .with_context("path", path)
                .with_context("deleted", self.processed.to_string())
        })
    }

    async fn flush(&mut self) -> Result<usize> {
        self.inner
            .flush()
            .await
            .map(|n| {
                self.processed += n as u64;
                n
            })
            .map_err(|err| {
                err.with_operation(Operation::DeleterFlush)
                    .with_context("service", self.scheme)
                    .with_context("deleted", self.processed.to_string())
            })
    }
}

impl<T: oio::BlockingDelete> oio::BlockingDelete for ErrorContextWrapper<T> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args).map_err(|err| {
            err.with_operation(Operation::DeleterDelete)
                .with_context("service", self.scheme)
                .with_context("path", path)
                .with_context("deleted", self.processed.to_string())
        })
    }

    fn flush(&mut self) -> Result<usize> {
        self.inner
            .flush()
            .map(|n| {
                self.processed += n as u64;
                n
            })
            .map_err(|err| {
                err.with_operation(Operation::DeleterFlush)
                    .with_context("service", self.scheme)
                    .with_context("deleted", self.processed.to_string())
            })
    }
}
