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
use std::time::Duration;

use opendal_core::raw::*;
use opendal_core::*;

/// `TimeoutLayer` adds deadlines to operations so slow or stalled work cannot
/// hang indefinitely.
///
/// For example, a dead connection could stall a database query. `TimeoutLayer`
/// interrupts the operation and returns an error that users can retry or report.
///
/// # Notes
///
/// `TimeoutLayer` applies two timeout budgets:
///
/// - `timeout` bounds control operations such as `stat`, `create_dir`, `rename`,
///   and `presign`.
/// - `io_timeout` bounds operations that open IO bodies, such as `read`, `write`,
///   and `list`, and every method call on returned readers, writers, listers,
///   deleters, and copiers.
///
/// # Default
///
/// - timeout: 60 seconds
/// - io_timeout: 10 seconds
///
/// # Cancellation Safety
///
/// `TimeoutLayer` enforces deadlines by dropping the in-flight future when a
/// timeout is reached. This can break lower layers that rely on a future being
/// resolved to restore internal state.
///
/// For example, while using `TimeoutLayer` with `RetryLayer` at the same time,
/// please make sure timeout layer is added before retry layer.
///
/// ```no_run
/// # use std::time::Duration;
/// #
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_retry::RetryLayer;
/// # use opendal_layer_timeout::TimeoutLayer;
/// #
/// # fn main() -> Result<()> {
/// let op = Operator::new(services::Memory::default())?
///     // This is fine: each retry attempt is timed out.
///     .layer(TimeoutLayer::default().with_io_timeout(Duration::from_nanos(1)))
///     .layer(RetryLayer::default())
///     // This is wrong: timeout can drop RetryLayer's future before it restores body state.
///     .layer(TimeoutLayer::default().with_io_timeout(Duration::from_nanos(1)));
/// # Ok(())
/// # }
/// ```
///
/// # Examples
///
/// The following example creates a timeout layer with a 10-second timeout for
/// control operations and a 3-second timeout for IO operations.
///
/// ```no_run
/// # use std::time::Duration;
/// #
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_timeout::TimeoutLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(
///         TimeoutLayer::default()
///             .with_timeout(Duration::from_secs(10))
///             .with_io_timeout(Duration::from_secs(3)),
///     );
/// # Ok(())
/// # }
/// ```
///
/// # Implementation Notes
///
/// `TimeoutLayer` uses [`tokio::time::timeout`] to bound service calls and IO
/// body methods. It also supplies an executor timeout so concurrent block write
/// and copy tasks can fail instead of waiting forever.
///
/// This introduces a small amount of overhead for IO operations, but it is needed
/// to implement timeouts correctly. OpenDAL used to implement this as a
/// zero-cost deadline check that only stored an [`Instant`] and compared it with
/// the current time. However, that approach does not work for all cases.
///
/// For example, a user's TCP connection could enter the
/// [Busy ESTAB](https://blog.cloudflare.com/when-tcp-sockets-refuse-to-die)
/// state. In this state, the connection emits no IO events, so the runtime never
/// polls the future again. The future hangs until Linux closes the connection
/// after it reaches the
/// [net.ipv4.tcp_retries2](https://man7.org/linux/man-pages/man7/tcp.7.html)
/// limit.
#[derive(Clone, Debug)]
pub struct TimeoutLayer {
    timeout: Duration,
    io_timeout: Duration,
}

impl Default for TimeoutLayer {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            io_timeout: Duration::from_secs(10),
        }
    }
}

impl TimeoutLayer {
    /// Create a new [`TimeoutLayer`] with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the timeout for control operations.
    ///
    /// This timeout is for all non-io operations like `stat`, `delete`.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the timeout for IO operations and body methods.
    ///
    /// This timeout is for all io operations like `read`, `Reader::read` and `Writer::write`.
    pub fn with_io_timeout(mut self, timeout: Duration) -> Self {
        self.io_timeout = timeout;
        self
    }
}

impl Layer for TimeoutLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }

    fn apply_context(&self, _srv: Servicer, inner: OperationContext) -> OperationContext {
        // Concurrent block IO paths read this timeout from the operation context's executor.
        let executor = Executor::with(TimeoutExecutor::new(
            inner.executor().clone().into_inner(),
            self.io_timeout,
        ));
        inner.with_executor(executor)
    }
}

impl TimeoutLayer {
    fn layer(&self, inner: Servicer) -> TimeoutService {
        TimeoutService {
            inner,
            timeout: self.timeout,
            io_timeout: self.io_timeout,
        }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct TimeoutService {
    inner: Servicer,
    timeout: Duration,
    io_timeout: Duration,
}

impl TimeoutService {
    async fn timeout<F: Future<Output = Result<T>>, T>(&self, op: Operation, fut: F) -> Result<T> {
        tokio::time::timeout(self.timeout, fut).await.map_err(|_| {
            Error::new(ErrorKind::Unexpected, "operation timeout reached")
                .with_operation(op)
                .with_context("timeout", self.timeout.as_secs_f64().to_string())
                .set_temporary()
        })?
    }
}

impl Service for TimeoutService {
    type Reader = TimeoutWrapper<oio::Reader>;
    type Writer = TimeoutWrapper<oio::Writer>;
    type Lister = TimeoutWrapper<oio::Lister>;
    type Deleter = TimeoutWrapper<oio::Deleter>;
    type Copier = TimeoutWrapper<oio::Copier>;
    type Composer = TimeoutWrapper<oio::Composer>;

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
        self.timeout(Operation::CreateDir, self.inner.create_dir(ctx, path, args))
            .await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        self.inner
            .read(ctx, path, args)
            .map(|r| TimeoutWrapper::new(r, self.io_timeout))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.inner
            .write(ctx, path, args)
            .map(|r| TimeoutWrapper::new(r, self.io_timeout))
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
            .map(|c| TimeoutWrapper::new(c, self.io_timeout))
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.inner
            .compose(ctx, to, args)
            .map(|c| TimeoutWrapper::new(c, self.io_timeout))
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.timeout(Operation::Rename, self.inner.rename(ctx, from, to, args))
            .await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.timeout(Operation::Restore, self.inner.restore(ctx, path, args))
            .await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.timeout(Operation::Stat, self.inner.stat(ctx, path, args))
            .await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner
            .delete(ctx)
            .map(|r| TimeoutWrapper::new(r, self.io_timeout))
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.inner
            .list(ctx, path, args)
            .map(|r| TimeoutWrapper::new(r, self.io_timeout))
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.timeout(Operation::Presign, self.inner.presign(ctx, path, args))
            .await
    }
}

struct TimeoutExecutor {
    exec: Arc<dyn Execute>,
    timeout: Duration,
}

impl TimeoutExecutor {
    fn new(exec: Arc<dyn Execute>, timeout: Duration) -> Self {
        Self { exec, timeout }
    }
}

impl Execute for TimeoutExecutor {
    fn execute(&self, f: BoxedStaticFuture<()>) {
        self.exec.execute(f)
    }

    fn timeout(&self) -> Option<BoxedStaticFuture<()>> {
        Some(Box::pin(tokio::time::sleep(self.timeout)))
    }
}

#[doc(hidden)]
pub struct TimeoutWrapper<R> {
    inner: R,

    timeout: Duration,
}

impl<R> TimeoutWrapper<R> {
    fn new(inner: R, timeout: Duration) -> Self {
        Self { inner, timeout }
    }

    #[inline]
    async fn io_timeout<F: Future<Output = Result<T>>, T>(
        timeout: Duration,
        op: &'static str,
        fut: F,
    ) -> Result<T> {
        tokio::time::timeout(timeout, fut).await.map_err(|_| {
            Error::new(ErrorKind::Unexpected, "io operation timeout reached")
                .with_operation(op)
                .with_context("timeout", timeout.as_secs_f64().to_string())
                .set_temporary()
        })?
    }
}

impl<R: oio::ReadStream> oio::ReadStream for TimeoutWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        let fut = self.inner.read();
        Self::io_timeout(self.timeout, Operation::Read.into_static(), fut).await
    }
}

impl<R: oio::Read> oio::Read for TimeoutWrapper<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let (rp, stream) = Self::io_timeout(
            self.timeout,
            Operation::Read.into_static(),
            self.inner.open(range),
        )
        .await?;
        Ok((
            rp,
            Box::new(TimeoutWrapper::new(stream, self.timeout)) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        Self::io_timeout(
            self.timeout,
            Operation::Read.into_static(),
            self.inner.read(range),
        )
        .await
    }
}

impl<R: oio::Write> oio::Write for TimeoutWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let fut = self.inner.write(bs);
        Self::io_timeout(self.timeout, Operation::Write.into_static(), fut).await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        let fut = self.inner.copy_from(path, args, range);
        Self::io_timeout(self.timeout, Operation::Write.into_static(), fut).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        let fut = self.inner.close();
        Self::io_timeout(self.timeout, Operation::Write.into_static(), fut).await
    }

    async fn abort(&mut self) -> Result<()> {
        let fut = self.inner.abort();
        Self::io_timeout(self.timeout, Operation::Write.into_static(), fut).await
    }
}

impl<R: oio::List> oio::List for TimeoutWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        let fut = self.inner.next();
        Self::io_timeout(self.timeout, Operation::List.into_static(), fut).await
    }
}

impl<R: oio::Delete> oio::Delete for TimeoutWrapper<R> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        let fut = self.inner.delete(path, args);
        Self::io_timeout(self.timeout, Operation::Delete.into_static(), fut).await
    }

    async fn close(&mut self) -> Result<()> {
        let fut = self.inner.close();
        Self::io_timeout(self.timeout, Operation::Delete.into_static(), fut).await
    }
}

impl<C: oio::Copy> oio::Copy for TimeoutWrapper<C> {
    async fn next(&mut self) -> Result<Option<usize>> {
        let fut = self.inner.next();
        Self::io_timeout(self.timeout, Operation::Copy.into_static(), fut).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        let fut = self.inner.close();
        Self::io_timeout(self.timeout, Operation::Copy.into_static(), fut).await
    }

    async fn abort(&mut self) -> Result<()> {
        let fut = self.inner.abort();
        Self::io_timeout(self.timeout, Operation::Copy.into_static(), fut).await
    }
}

impl<C: oio::Compose> oio::Compose for TimeoutWrapper<C> {
    async fn compose(&mut self, path: &str, args: OpRead) -> Result<()> {
        let fut = self.inner.compose(path, args);
        Self::io_timeout(self.timeout, Operation::Compose.into_static(), fut).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        let fut = self.inner.close();
        Self::io_timeout(self.timeout, Operation::Compose.into_static(), fut).await
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use futures::StreamExt;
    use tokio::time::timeout;

    use super::*;

    #[derive(Debug, Clone, Default)]
    struct MockService;

    impl Service for MockService {
        type Reader = MockReader;
        type Writer = ();
        type Lister = MockLister;
        type Deleter = MockDeleter;
        type Copier = MockCopier;
        type Composer = ();

        fn info(&self) -> ServiceInfo {
            ServiceInfo::with_scheme("mock")
        }

        fn capability(&self) -> Capability {
            Capability {
                read: true,
                delete: true,
                list: true,
                copy: true,
                ..Default::default()
            }
        }

        async fn create_dir(
            &self,
            _: &OperationContext,
            _: &str,
            _: OpCreateDir,
        ) -> Result<RpCreateDir> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        /// Return a reader whose operations never complete.
        fn read(&self, _ctx: &OperationContext, _: &str, _: OpRead) -> Result<Self::Reader> {
            Ok(MockReader)
        }

        fn write(&self, _ctx: &OperationContext, _: &str, _: OpWrite) -> Result<Self::Writer> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        /// Return a deleter whose operations never complete.
        fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
            Ok(MockDeleter)
        }

        fn list(&self, _ctx: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
            Ok(MockLister)
        }

        fn copy(&self, _: &OperationContext, _: &str, _: &str, _: OpCopy) -> Result<Self::Copier> {
            Ok(MockCopier)
        }

        async fn rename(
            &self,
            _: &OperationContext,
            _: &str,
            _: &str,
            _: OpRename,
        ) -> Result<RpRename> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }

        async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
            Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            ))
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockReader;

    impl oio::Read for MockReader {
        async fn open(&self, _: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
            pending().await
        }

        async fn read(&self, _: BytesRange) -> Result<(RpRead, Buffer)> {
            pending().await
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockLister;

    impl oio::List for MockLister {
        async fn next(&mut self) -> Result<Option<oio::Entry>> {
            pending().await
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockDeleter;

    impl oio::Delete for MockDeleter {
        async fn delete(&mut self, _: &str, _: OpDelete) -> Result<()> {
            pending().await
        }

        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockCopier;

    impl oio::Copy for MockCopier {
        async fn next(&mut self) -> Result<Option<usize>> {
            pending().await
        }

        async fn close(&mut self) -> Result<Metadata> {
            pending().await
        }

        async fn abort(&mut self) -> Result<()> {
            pending().await
        }
    }

    #[tokio::test]
    async fn test_delete_timeout() {
        let srv = MockService;
        let op = Operator::from_parts(OperationContext::default(), Arc::new(srv))
            .layer(TimeoutLayer::default().with_io_timeout(Duration::from_secs(1)));

        let fut = async {
            let res = op.delete("test").await;
            assert!(res.is_err());
            let err = res.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::Unexpected);
            assert!(err.to_string().contains("timeout"))
        };

        timeout(Duration::from_secs(2), fut)
            .await
            .expect("this test should not exceed 2 seconds")
    }

    #[tokio::test]
    async fn test_io_timeout() {
        let srv = MockService;
        let op = Operator::from_parts(OperationContext::default(), Arc::new(srv))
            .layer(TimeoutLayer::default().with_io_timeout(Duration::from_secs(1)));

        let reader = op.reader("test").await.unwrap();

        let res = reader.read(0..4).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(err.to_string().contains("timeout"))
    }

    #[tokio::test]
    async fn test_list_timeout() {
        let srv = MockService;
        let op = Operator::from_parts(OperationContext::default(), Arc::new(srv)).layer(
            TimeoutLayer::default()
                .with_timeout(Duration::from_secs(1))
                .with_io_timeout(Duration::from_secs(1)),
        );

        let mut lister = op.lister("test").await.unwrap();

        let res = lister.next().await.unwrap();
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(err.to_string().contains("timeout"))
    }

    #[tokio::test]
    async fn test_delete_io_timeout() {
        use oio::Delete;

        let mut deleter = TimeoutWrapper::new(MockDeleter, Duration::from_secs(1));

        let res = deleter.delete("test", OpDelete::default()).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(err.to_string().contains("timeout"));
    }

    #[tokio::test]
    async fn test_copy_io_timeout() {
        use oio::Copy;

        let service = TimeoutLayer::default()
            .with_io_timeout(Duration::from_millis(100))
            .apply_service(Arc::new(MockService));
        let ctx = OperationContext::new();
        let mut copier = service.copy(&ctx, "f", "t", OpCopy::default()).unwrap();

        let err = copier.next().await.unwrap_err();
        assert!(err.to_string().contains("timeout"));
    }

    #[tokio::test]
    async fn test_list_timeout_raw() {
        use oio::List;

        let timeout_layer = TimeoutLayer::default()
            .with_timeout(Duration::from_secs(1))
            .with_io_timeout(Duration::from_secs(1));
        let service = timeout_layer.apply_service(Arc::new(MockService));
        let ctx = OperationContext::new();

        let mut lister = service.list(&ctx, "test", OpList::default()).unwrap();

        let res = lister.next().await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(err.to_string().contains("timeout"));
    }
}
