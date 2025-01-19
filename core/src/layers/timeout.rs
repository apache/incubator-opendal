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
use std::time::Duration;

use crate::raw::*;
use crate::*;

/// Add timeout for every operation to avoid slow or unexpected hang operations.
///
/// For example, a dead connection could hang a databases sql query. TimeoutLayer
/// will break this connection and returns an error so users can handle it by
/// retrying or print to users.
///
/// # Notes
///
/// `TimeoutLayer` treats all operations in two kinds:
///
/// - Non IO Operation like `stat`, `delete` they operate on a single file. We control
///   them by setting `timeout`.
/// - IO Operation like `read`, `Reader::read` and `Writer::write`, they operate on data directly, we
///   control them by setting `io_timeout`.
///
/// # Default
///
/// - timeout: 60 seconds
/// - io_timeout: 10 seconds
///
/// # Panics
///
/// TimeoutLayer will drop the future if the timeout is reached. This might cause the internal state
/// of the future to be broken. If underlying future moves ownership into the future, it will be
/// dropped and will neven return back.
///
/// For example, while using `TimeoutLayer` with `RetryLayer` at the same time, please make sure
/// timeout layer showed up before retry layer.
///
/// ```no_run
/// # use std::time::Duration;
///
/// # use opendal::layers::RetryLayer;
/// # use opendal::layers::TimeoutLayer;
/// # use opendal::services;
/// # use opendal::Operator;
/// # use opendal::Result;
///
/// # fn main() -> Result<()> {
/// let op = Operator::new(services::Memory::default())?
///     // This is fine, since timeout happen during retry.
///     .layer(TimeoutLayer::new().with_io_timeout(Duration::from_nanos(1)))
///     .layer(RetryLayer::new())
///     // This is wrong. Since timeout layer will drop future, leaving retry layer in a bad state.
///     .layer(TimeoutLayer::new().with_io_timeout(Duration::from_nanos(1)))
///     .finish();
/// Ok(())
/// # }
/// ```
///
/// # Examples
///
/// The following examples will create a timeout layer with 10 seconds timeout for all non-io
/// operations, 3 seconds timeout for all io operations.
///
/// ```no_run
/// # use std::time::Duration;
///
/// # use opendal::layers::TimeoutLayer;
/// # use opendal::services;
/// # use opendal::Operator;
/// # use opendal::Result;
/// # use opendal::Scheme;
///
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(
///         TimeoutLayer::default()
///             .with_timeout(Duration::from_secs(10))
///             .with_io_timeout(Duration::from_secs(3)),
///     )
///     .finish();
/// Ok(())
/// # }
/// ```
///
/// # Implementation Notes
///
/// TimeoutLayer is using [`tokio::time::timeout`] to implement timeout for operations. And IO
/// Operations insides `reader`, `writer` will use `Pin<Box<tokio::time::Sleep>>` to track the
/// timeout.
///
/// This might introduce a bit overhead for IO operations, but it's the only way to implement
/// timeout correctly. We used to implement timeout layer in zero cost way that only stores
/// a [`std::time::Instant`] and check the timeout by comparing the instant with current time.
/// However, it doesn't work for all cases.
///
/// For examples, users TCP connection could be in [Busy ESTAB](https://blog.cloudflare.com/when-tcp-sockets-refuse-to-die) state. In this state, no IO event will be emitted. The runtime
/// will never poll our future again. From the application side, this future is hanging forever
/// until this TCP connection is closed for reaching the linux [net.ipv4.tcp_retries2](https://man7.org/linux/man-pages/man7/tcp.7.html) times.
#[derive(Clone)]
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
    /// Create a new `TimeoutLayer` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set timeout for TimeoutLayer with given value.
    ///
    /// This timeout is for all non-io operations like `stat`, `delete`.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set io timeout for TimeoutLayer with given value.
    ///
    /// This timeout is for all io operations like `read`, `Reader::read` and `Writer::write`.
    pub fn with_io_timeout(mut self, timeout: Duration) -> Self {
        self.io_timeout = timeout;
        self
    }

    /// Set speed for TimeoutLayer with given value.
    ///
    /// # Notes
    ///
    /// The speed should be the lower bound of the IO speed. Set this value too
    /// large could result in all write operations failing.
    ///
    /// # Panics
    ///
    /// This function will panic if speed is 0.
    #[deprecated(note = "with speed is not supported anymore, please use with_io_timeout instead")]
    pub fn with_speed(self, _: u64) -> Self {
        self
    }
}

impl<A: Access> Layer<A> for TimeoutLayer {
    type LayeredAccess = TimeoutAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        TimeoutAccessor {
            inner,

            timeout: self.timeout,
            io_timeout: self.io_timeout,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeoutAccessor<A: Access> {
    inner: A,

    timeout: Duration,
    io_timeout: Duration,
}

impl<A: Access> TimeoutAccessor<A> {
    async fn timeout<F: Future<Output = Result<T>>, T>(&self, op: Operation, fut: F) -> Result<T> {
        tokio::time::timeout(self.timeout, fut).await.map_err(|_| {
            Error::new(ErrorKind::Unexpected, "operation timeout reached")
                .with_operation(op)
                .with_context("timeout", self.timeout.as_secs_f64().to_string())
                .set_temporary()
        })?
    }

    async fn io_timeout<F: Future<Output = Result<T>>, T>(
        &self,
        op: Operation,
        fut: F,
    ) -> Result<T> {
        tokio::time::timeout(self.io_timeout, fut)
            .await
            .map_err(|_| {
                Error::new(ErrorKind::Unexpected, "io timeout reached")
                    .with_operation(op)
                    .with_context("timeout", self.io_timeout.as_secs_f64().to_string())
                    .set_temporary()
            })?
    }
}

impl<A: Access> LayeredAccess for TimeoutAccessor<A> {
    type Inner = A;
    type Reader = TimeoutWrapper<A::Reader>;
    type BlockingReader = A::BlockingReader;
    type Writer = TimeoutWrapper<A::Writer>;
    type BlockingWriter = A::BlockingWriter;
    type Lister = TimeoutWrapper<A::Lister>;
    type BlockingLister = A::BlockingLister;
    type Deleter = TimeoutWrapper<A::Deleter>;
    type BlockingDeleter = A::BlockingDeleter;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        self.timeout(Operation::CreateDir, self.inner.create_dir(path, args))
            .await
    }

    async fn read(&self, path: &str, mut args: OpRead) -> Result<(RpRead, Self::Reader)> {
        if let Some(exec) = args.executor().cloned() {
            args = args.with_executor(Executor::with(TimeoutExecutor::new(
                exec.into_inner(),
                self.io_timeout,
            )));
        }

        self.io_timeout(Operation::Read, self.inner.read(path, args))
            .await
            .map(|(rp, r)| (rp, TimeoutWrapper::new(r, self.io_timeout)))
    }

    async fn write(&self, path: &str, mut args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        if let Some(exec) = args.executor().cloned() {
            args = args.with_executor(Executor::with(TimeoutExecutor::new(
                exec.into_inner(),
                self.io_timeout,
            )));
        }

        self.io_timeout(Operation::Write, self.inner.write(path, args))
            .await
            .map(|(rp, r)| (rp, TimeoutWrapper::new(r, self.io_timeout)))
    }

    async fn copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        self.timeout(Operation::Copy, self.inner.copy(from, to, args))
            .await
    }

    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.timeout(Operation::Rename, self.inner.rename(from, to, args))
            .await
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.timeout(Operation::Stat, self.inner.stat(path, args))
            .await
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        self.timeout(Operation::Delete, self.inner.delete())
            .await
            .map(|(rp, r)| (rp, TimeoutWrapper::new(r, self.io_timeout)))
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        self.io_timeout(Operation::List, self.inner.list(path, args))
            .await
            .map(|(rp, r)| (rp, TimeoutWrapper::new(r, self.io_timeout)))
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        self.timeout(Operation::Presign, self.inner.presign(path, args))
            .await
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        self.inner.blocking_read(path, args)
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        self.inner.blocking_write(path, args)
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        self.inner.blocking_list(path, args)
    }

    fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
        self.inner.blocking_delete()
    }
}

pub struct TimeoutExecutor {
    exec: Arc<dyn Execute>,
    timeout: Duration,
}

impl TimeoutExecutor {
    pub fn new(exec: Arc<dyn Execute>, timeout: Duration) -> Self {
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

impl<R: oio::Read> oio::Read for TimeoutWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        let fut = self.inner.read();
        Self::io_timeout(self.timeout, Operation::ReaderRead.into_static(), fut).await
    }
}

impl<R: oio::Write> oio::Write for TimeoutWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let fut = self.inner.write(bs);
        Self::io_timeout(self.timeout, Operation::WriterWrite.into_static(), fut).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        let fut = self.inner.close();
        Self::io_timeout(self.timeout, Operation::WriterClose.into_static(), fut).await
    }

    async fn abort(&mut self) -> Result<()> {
        let fut = self.inner.abort();
        Self::io_timeout(self.timeout, Operation::WriterAbort.into_static(), fut).await
    }
}

impl<R: oio::List> oio::List for TimeoutWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        let fut = self.inner.next();
        Self::io_timeout(self.timeout, Operation::ListerNext.into_static(), fut).await
    }
}

impl<R: oio::Delete> oio::Delete for TimeoutWrapper<R> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args)
    }

    async fn flush(&mut self) -> Result<usize> {
        let fut = self.inner.flush();
        Self::io_timeout(self.timeout, Operation::DeleterFlush.into_static(), fut).await
    }
}

#[cfg(test)]
mod tests {
    use std::future::pending;
    use std::future::Future;
    use std::sync::Arc;
    use std::time::Duration;

    use futures::StreamExt;
    use tokio::time::sleep;
    use tokio::time::timeout;

    use crate::layers::TimeoutLayer;
    use crate::layers::TypeEraseLayer;
    use crate::raw::*;
    use crate::*;

    #[derive(Debug, Clone, Default)]
    struct MockService;

    impl Access for MockService {
        type Reader = MockReader;
        type Writer = ();
        type Lister = MockLister;
        type BlockingReader = ();
        type BlockingWriter = ();
        type BlockingLister = ();
        type Deleter = ();
        type BlockingDeleter = ();

        fn info(&self) -> Arc<AccessorInfo> {
            let mut am = AccessorInfo::default();
            am.set_native_capability(Capability {
                read: true,
                delete: true,
                ..Default::default()
            });

            am.into()
        }

        /// This function will build a reader that always return pending.
        async fn read(&self, _: &str, _: OpRead) -> Result<(RpRead, Self::Reader)> {
            Ok((RpRead::new(), MockReader))
        }

        /// This function will never return.
        async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
            sleep(Duration::from_secs(u64::MAX)).await;

            Ok((RpDelete::default(), ()))
        }

        async fn list(&self, _: &str, _: OpList) -> Result<(RpList, Self::Lister)> {
            Ok((RpList::default(), MockLister))
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockReader;

    impl oio::Read for MockReader {
        fn read(&mut self) -> impl Future<Output = Result<Buffer>> {
            pending()
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MockLister;

    impl oio::List for MockLister {
        fn next(&mut self) -> impl Future<Output = Result<Option<oio::Entry>>> {
            pending()
        }
    }

    #[tokio::test]
    async fn test_operation_timeout() {
        let acc = Arc::new(TypeEraseLayer.layer(MockService)) as Accessor;
        let op = Operator::from_inner(acc)
            .layer(TimeoutLayer::new().with_timeout(Duration::from_secs(1)));

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
        let acc = Arc::new(TypeEraseLayer.layer(MockService)) as Accessor;
        let op = Operator::from_inner(acc)
            .layer(TimeoutLayer::new().with_io_timeout(Duration::from_secs(1)));

        let reader = op.reader("test").await.unwrap();

        let res = reader.read(0..4).await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(err.to_string().contains("timeout"))
    }

    #[tokio::test]
    async fn test_list_timeout() {
        let acc = Arc::new(TypeEraseLayer.layer(MockService)) as Accessor;
        let op = Operator::from_inner(acc).layer(
            TimeoutLayer::new()
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
    async fn test_list_timeout_raw() {
        use oio::List;

        let acc = MockService;
        let timeout_layer = TimeoutLayer::new()
            .with_timeout(Duration::from_secs(1))
            .with_io_timeout(Duration::from_secs(1));
        let timeout_acc = timeout_layer.layer(acc);

        let (_, mut lister) = Access::list(&timeout_acc, "test", OpList::default())
            .await
            .unwrap();

        let res = lister.next().await;
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(err.to_string().contains("timeout"));
    }
}
