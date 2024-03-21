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

use std::io;
use std::pin::pin;
use std::pin::Pin;
use std::task::ready;
use std::task::Context;
use std::task::Poll;

use bytes::Buf;
use bytes::Bytes;
use futures::TryStreamExt;

use crate::raw::oio::Write;
use crate::raw::*;
use crate::*;

/// Writer is designed to write data into given path in an asynchronous
/// manner.
///
/// ## Notes
///
/// Please make sure either `close` or `abort` has been called before
/// dropping the writer otherwise the data could be lost.
///
/// ## Usage
///
/// ### Write Multiple Chunks
///
/// Some services support to write multiple chunks of data into given path. Services that doesn't
/// support write multiple chunks will return [`ErrorKind::Unsupported`] error when calling `write`
/// at the second time.
///
/// ```no_build
/// let mut w = op.writer("path/to/file").await?;
/// w.write(bs).await?;
/// w.write(bs).await?;
/// w.close().await?
/// ```
///
/// Our writer also provides [`Writer::sink`] and [`Writer::copy`] support.
///
/// Besides, our writer implements [`AsyncWrite`] and [`tokio::io::AsyncWrite`].
///
/// ### Write with append enabled
///
/// Writer also supports to write with append enabled. This is useful when users want to append
/// some data to the end of the file.
///
/// - If file doesn't exist, it will be created and just like calling `write`.
/// - If file exists, data will be appended to the end of the file.
///
/// Possible Errors:
///
/// - Some services store normal file and appendable file in different way. Trying to append
///   on non-appendable file could return [`ErrorKind::ConditionNotMatch`] error.
/// - Services that doesn't support append will return [`ErrorKind::Unsupported`] error when
///   creating writer with `append` enabled.
pub struct Writer {
    state: State,
}

impl Writer {
    /// Create a new writer from an `oio::Writer`.
    pub(crate) fn new(w: oio::Writer) -> Self {
        Writer {
            state: State::Idle(Some(w)),
        }
    }

    /// Create a new writer.
    ///
    /// Create will use internal information to decide the most suitable
    /// implementation for users.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) async fn create(acc: FusedAccessor, path: &str, op: OpWrite) -> Result<Self> {
        let (_, w) = acc.write(path, op).await?;

        Ok(Writer {
            state: State::Idle(Some(w)),
        })
    }

    /// Write into inner writer.
    pub async fn write(&mut self, bs: impl Into<Bytes>) -> Result<()> {
        let State::Idle(Some(w)) = &mut self.state else {
            return Err(Error::new(ErrorKind::Unexpected, "writer must be valid"));
        };

        let mut bs = bs.into();
        while !bs.is_empty() {
            let n = w.write(bs.clone()).await?;
            bs.advance(n);
        }

        Ok(())
    }

    /// Sink into writer.
    ///
    /// sink will read data from given streamer and write them into writer
    /// directly without extra in-memory buffer.
    ///
    /// # Notes
    ///
    /// - Sink doesn't support to be used with write concurrently.
    /// - Sink doesn't support to be used without content length now.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use bytes::Bytes;
    /// use futures::stream;
    /// use futures::StreamExt;
    /// use opendal::Operator;
    /// use opendal::Result;
    ///
    /// async fn sink_example(op: Operator) -> Result<()> {
    ///     let mut w = op.writer_with("path/to/file").await?;
    ///     let stream = stream::iter(vec![vec![0; 4096], vec![1; 4096]]).map(Ok);
    ///     w.sink(stream).await?;
    ///     w.close().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn sink<S, T>(&mut self, sink_from: S) -> Result<u64>
    where
        S: futures::Stream<Item = Result<T>>,
        T: Into<Bytes>,
    {
        let State::Idle(Some(w)) = &mut self.state else {
            return Err(Error::new(ErrorKind::Unexpected, "writer must be valid"));
        };

        let mut sink_from = pin!(sink_from);
        let mut written = 0;
        while let Some(bs) = sink_from.try_next().await? {
            let mut bs = bs.into();
            while !bs.is_empty() {
                let n = w.write(bs.clone()).await?;
                bs.advance(n);
                written += n as u64;
            }
        }
        Ok(written)
    }

    /// Copy into writer.
    ///
    /// copy will read data from given reader and write them into writer
    /// directly with only one constant in-memory buffer.
    ///
    /// # Notes
    ///
    /// - Copy doesn't support to be used with write concurrently.
    /// - Copy doesn't support to be used without content length now.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use bytes::Bytes;
    /// use futures::io::Cursor;
    /// use futures::stream;
    /// use futures::StreamExt;
    /// use opendal::Operator;
    /// use opendal::Result;
    ///
    /// async fn copy_example(op: Operator) -> Result<()> {
    ///     let mut w = op.writer_with("path/to/file").await?;
    ///     let reader = Cursor::new(vec![0; 4096]);
    ///     w.copy(reader).await?;
    ///     w.close().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn copy<R>(&mut self, read_from: R) -> Result<u64>
    where
        R: futures::AsyncRead,
    {
        futures::io::copy(read_from, self).await.map_err(|err| {
            Error::new(ErrorKind::Unexpected, "copy into writer failed")
                .with_operation("copy")
                .set_source(err)
        })
    }

    /// Abort the writer and clean up all written data.
    ///
    /// ## Notes
    ///
    /// Abort should only be called when the writer is not closed or
    /// aborted, otherwise an unexpected error could be returned.
    pub async fn abort(&mut self) -> Result<()> {
        let State::Idle(Some(w)) = &mut self.state else {
            return Err(Error::new(ErrorKind::Unexpected, "writer must be valid"));
        };

        w.abort().await
    }

    /// Close the writer and make sure all data have been committed.
    ///
    /// ## Notes
    ///
    /// Close should only be called when the writer is not closed or
    /// aborted, otherwise an unexpected error could be returned.
    pub async fn close(&mut self) -> Result<()> {
        let State::Idle(Some(w)) = &mut self.state else {
            return Err(Error::new(ErrorKind::Unexpected, "writer must be valid"));
        };

        w.close().await
    }
}

enum State {
    Idle(Option<oio::Writer>),
    Writing(BoxedStaticFuture<(oio::Writer, Result<usize>)>),
    Closing(BoxedStaticFuture<(oio::Writer, Result<()>)>),
}

unsafe impl Sync for State {}

impl futures::AsyncWrite for Writer {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut self.state {
            State::Idle(w) => {
                let mut w = w.take().expect("writer must be valid");
                let bs = Bytes::copy_from_slice(buf);
                let fut = async move {
                    let res = w.write(bs).await;
                    (w, res)
                };
                self.state = State::Writing(Box::pin(fut));
                self.poll_write(cx, buf)
            }
            State::Writing(fut) => {
                let (w, res) = ready!(fut.as_mut().poll(cx));
                self.state = State::Idle(Some(w));
                Poll::Ready(res.map_err(format_std_io_error))
            }
            _ => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "another io operation is in progress",
            ))),
        }
    }

    /// Writer makes sure that every write is flushed.
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut self.state {
            State::Idle(w) => {
                let mut w = w.take().expect("writer must be valid");
                let fut = async move {
                    let res = w.close().await;
                    (w, res)
                };
                self.state = State::Closing(Box::pin(fut));
                self.poll_close(cx)
            }
            State::Closing(fut) => {
                let (w, res) = ready!(fut.as_mut().poll(cx));
                self.state = State::Idle(Some(w));
                Poll::Ready(res.map_err(format_std_io_error))
            }
            _ => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "another io operation is in progress",
            ))),
        }
    }
}

impl tokio::io::AsyncWrite for Writer {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut self.state {
            State::Idle(w) => {
                let mut w = w.take().expect("writer must be valid");
                let bs = Bytes::copy_from_slice(buf);
                let fut = async move {
                    let res = w.write(bs).await;
                    (w, res)
                };
                self.state = State::Writing(Box::pin(fut));
                self.poll_write(cx, buf)
            }
            State::Writing(fut) => {
                let (w, res) = ready!(fut.as_mut().poll(cx));
                self.state = State::Idle(Some(w));
                Poll::Ready(res.map_err(format_std_io_error))
            }
            _ => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "another io operation is in progress",
            ))),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut self.state {
            State::Idle(w) => {
                let mut w = w.take().expect("writer must be valid");
                let fut = async move {
                    let res = w.close().await;
                    (w, res)
                };
                self.state = State::Closing(Box::pin(fut));
                self.poll_shutdown(cx)
            }
            State::Closing(fut) => {
                let (w, res) = ready!(fut.as_mut().poll(cx));
                self.state = State::Idle(Some(w));
                Poll::Ready(res.map_err(format_std_io_error))
            }
            _ => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "another io operation is in progress",
            ))),
        }
    }
}

/// BlockingWriter is designed to write data into given path in an blocking
/// manner.
pub struct BlockingWriter {
    pub(crate) inner: oio::BlockingWriter,
}

impl BlockingWriter {
    /// Create a new writer.
    ///
    /// Create will use internal information to decide the most suitable
    /// implementation for users.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) fn create(acc: FusedAccessor, path: &str, op: OpWrite) -> Result<Self> {
        let (_, w) = acc.blocking_write(path, op)?;

        Ok(BlockingWriter { inner: w })
    }

    /// Write into inner writer.
    pub fn write(&mut self, bs: impl Into<Bytes>) -> Result<()> {
        let mut bs = bs.into();
        while !bs.is_empty() {
            let n = self.inner.write(bs.clone())?;
            bs.advance(n);
        }

        Ok(())
    }

    /// Close the writer and make sure all data have been stored.
    pub fn close(&mut self) -> Result<()> {
        self.inner.close()
    }
}

impl io::Write for BlockingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner
            .write(Bytes::copy_from_slice(buf))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
