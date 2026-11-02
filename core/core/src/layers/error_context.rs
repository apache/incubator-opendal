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
/// - `service`: The scheme of underlying service.
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

impl std::fmt::Debug for ErrorContextLayer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErrorContextLayer").finish()
    }
}

impl Layer for ErrorContextLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl ErrorContextLayer {
    fn layer(&self, inner: Servicer) -> ErrorContextService {
        ErrorContextService { inner }
    }
}

/// Provide error context wrapper for backend.
pub struct ErrorContextService {
    inner: Servicer,
}

impl std::fmt::Debug for ErrorContextService {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl Service for ErrorContextService {
    type Reader = ErrorContextWrapper<oio::Reader>;
    type Writer = ErrorContextWrapper<oio::Writer>;
    type Lister = ErrorContextWrapper<oio::Lister>;
    type Deleter = ErrorContextWrapper<oio::Deleter>;
    type Copier = ErrorContextWrapper<oio::Copier>;
    type Composer = ErrorContextWrapper<oio::Composer>;

    fn info(&self) -> ServiceInfo {
        self.inner.info()
    }

    fn capability(&self) -> Capability {
        self.inner.capability()
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.inner.create_dir(ctx, path, args).await.map_err(|err| {
            err.with_operation(Operation::CreateDir)
                .with_context("service", self.info().scheme())
                .with_context("path", path)
        })
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        self.inner
            .read(ctx, path, args)
            .map(|r| ErrorContextWrapper::new(self.info().scheme(), path, r))
            .map_err(|err| {
                err.with_operation(Operation::Read)
                    .with_context("service", self.info().scheme())
                    .with_context("path", path)
            })
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.inner
            .write(ctx, path, args)
            .map(|w| ErrorContextWrapper::new(self.info().scheme(), path, w))
            .map_err(|err| {
                err.with_operation(Operation::Write)
                    .with_context("service", self.info().scheme())
                    .with_context("path", path)
            })
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
            .map(|p| ErrorContextWrapper::new(self.info().scheme(), to, p))
            .map_err(|err| {
                err.with_operation(Operation::Copy)
                    .with_context("service", self.info().scheme())
                    .with_context("from", from)
                    .with_context("to", to)
            })
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.inner
            .compose(ctx, to, args)
            .map(|composer| ErrorContextWrapper::new(self.info().scheme(), to, composer))
            .map_err(|err| {
                err.with_operation(Operation::Compose)
                    .with_context("service", self.info().scheme())
                    .with_context("to", to)
            })
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.inner.rename(ctx, from, to, args).await.map_err(|err| {
            err.with_operation(Operation::Rename)
                .with_context("service", self.info().scheme())
                .with_context("from", from)
                .with_context("to", to)
        })
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.inner.restore(ctx, path, args).await.map_err(|err| {
            err.with_operation(Operation::Restore)
                .with_context("service", self.info().scheme())
                .with_context("path", path)
        })
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(ctx, path, args).await.map_err(|err| {
            err.with_operation(Operation::Stat)
                .with_context("service", self.info().scheme())
                .with_context("path", path)
        })
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner
            .delete(ctx)
            .map(|w| ErrorContextWrapper::new(self.info().scheme(), "", w))
            .map_err(|err| {
                err.with_operation(Operation::Delete)
                    .with_context("service", self.info().scheme())
            })
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.inner
            .list(ctx, path, args)
            .map(|p| ErrorContextWrapper::new(self.info().scheme(), path, p))
            .map_err(|err| {
                err.with_operation(Operation::List)
                    .with_context("service", self.info().scheme())
                    .with_context("path", path)
            })
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.inner.presign(ctx, path, args).await.map_err(|err| {
            err.with_operation(Operation::Presign)
                .with_context("service", self.info().scheme())
                .with_context("path", path)
        })
    }
}

pub struct ErrorContextWrapper<T> {
    scheme: &'static str,
    path: String,
    inner: T,
    range: BytesRange,
    processed: u64,
}

impl<T> ErrorContextWrapper<T> {
    fn new(scheme: &'static str, path: impl Into<String>, inner: T) -> Self {
        Self {
            scheme,
            path: path.into(),
            inner,
            range: BytesRange::default(),
            processed: 0,
        }
    }

    fn with_range(mut self, range: BytesRange) -> Self {
        self.range = range;
        self
    }

    fn read_error(&self, err: Error) -> Error {
        err.with_operation(Operation::Read)
            .with_context("service", self.scheme)
            .with_context("path", &self.path)
            .with_context("range", self.range.to_string())
            .with_context("read", self.processed.to_string())
    }
}

impl<T: oio::Read> oio::Read for ErrorContextWrapper<T> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        self.inner
            .open(range)
            .await
            .map(|(rp, stream)| {
                (
                    rp,
                    Box::new(
                        ErrorContextWrapper::new(self.scheme, self.path.clone(), stream)
                            .with_range(range),
                    ) as Box<dyn oio::ReadStreamDyn>,
                )
            })
            .map_err(|err| {
                err.with_operation(Operation::Read)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("range", range.to_string())
            })
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        self.inner.read(range).await.map_err(|err| {
            err.with_operation(Operation::Read)
                .with_context("service", self.scheme)
                .with_context("path", &self.path)
                .with_context("range", range.to_string())
        })
    }
}

impl<T: oio::ReadStream> oio::ReadStream for ErrorContextWrapper<T> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner
            .read()
            .await
            .inspect(|bs| {
                self.processed += bs.len() as u64;
            })
            .map_err(|err| self.read_error(err))
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
                err.with_operation(Operation::Write)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("size", size.to_string())
                    .with_context("written", self.processed.to_string())
            })
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        let size = range.size().ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "native writer copy requires a bounded range",
            )
        })?;
        self.inner
            .copy_from(path, args, range)
            .await
            .inspect(|_| {
                self.processed += size;
            })
            .map_err(|err| {
                err.with_operation(Operation::Write)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("source", path)
                    .with_context("range", range.to_string())
                    .with_context("written", self.processed.to_string())
            })
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().await.map_err(|err| {
            err.with_operation(Operation::Write)
                .with_context("service", self.scheme)
                .with_context("path", &self.path)
                .with_context("written", self.processed.to_string())
        })
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner.abort().await.map_err(|err| {
            err.with_operation(Operation::Write)
                .with_context("service", self.scheme)
                .with_context("path", &self.path)
                .with_context("processed", self.processed.to_string())
        })
    }
}

impl<T: oio::List> oio::List for ErrorContextWrapper<T> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner
            .next()
            .await
            .inspect(|bs| {
                self.processed += bs.is_some() as u64;
            })
            .map_err(|err| {
                err.with_operation(Operation::List)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("listed", self.processed.to_string())
            })
    }
}

impl<T: oio::Delete> oio::Delete for ErrorContextWrapper<T> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args).await.map_err(|err| {
            err.with_operation(Operation::Delete)
                .with_context("service", self.scheme)
                .with_context("path", path)
                .with_context("deleted", self.processed.to_string())
        })
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.close().await.map_err(|err| {
            err.with_operation(Operation::Delete)
                .with_context("service", self.scheme)
                .with_context("deleted", self.processed.to_string())
        })
    }
}

impl<T: oio::Copy> oio::Copy for ErrorContextWrapper<T> {
    async fn next(&mut self) -> Result<Option<usize>> {
        self.inner
            .next()
            .await
            .inspect(|size| {
                self.processed += size.unwrap_or_default() as u64;
            })
            .map_err(|err| {
                err.with_operation(Operation::Copy)
                    .with_context("service", self.scheme)
                    .with_context("path", &self.path)
                    .with_context("copied", self.processed.to_string())
            })
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().await.map_err(|err| {
            err.with_operation(Operation::Copy)
                .with_context("service", self.scheme)
                .with_context("path", &self.path)
                .with_context("copied", self.processed.to_string())
        })
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner.abort().await.map_err(|err| {
            err.with_operation(Operation::Copy)
                .with_context("service", self.scheme)
                .with_context("path", &self.path)
                .with_context("copied", self.processed.to_string())
        })
    }
}

impl<T: oio::Compose> oio::Compose for ErrorContextWrapper<T> {
    async fn compose(&mut self, path: &str, args: OpRead) -> Result<()> {
        self.inner
            .compose(path, args)
            .await
            .inspect(|_| {
                self.processed += 1;
            })
            .map_err(|err| {
                err.with_operation(Operation::Compose)
                    .with_context("service", self.scheme)
                    .with_context("from", path)
                    .with_context("to", &self.path)
                    .with_context("accepted", self.processed.to_string())
            })
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().await.map_err(|err| {
            err.with_operation(Operation::Compose)
                .with_context("service", self.scheme)
                .with_context("to", &self.path)
                .with_context("accepted", self.processed.to_string())
        })
    }
}
