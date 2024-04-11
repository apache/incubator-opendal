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

use std::ops::Bound;
use std::ops::Range;
use std::ops::RangeBounds;

use bytes::Buf;
use bytes::BufMut;

use crate::raw::*;
use crate::*;

/// Reader is designed to read data from given path in an asynchronous
/// manner.
///
/// # Usage
///
/// [`Reader`] provides multiple ways to read data from given reader. Please note that it's
/// undefined behavior to use `Reader` in different ways.
///
/// ## Direct
///
/// [`Reader`] provides public API including [`Reader::read`], [`Reader:read_range`], and [`Reader::read_to_end`]. You can use those APIs directly without extra copy.
pub struct Reader {
    inner: oio::Reader,
}

impl Reader {
    /// Create a new reader.
    ///
    /// Create will use internal information to decide the most suitable
    /// implementation for users.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) async fn create(acc: FusedAccessor, path: &str, op: OpRead) -> Result<Self> {
        let (_, r) = acc.read(path, op).await?;

        Ok(Reader { inner: r })
    }

    /// Read give range from reader into [`Buffer`].
    ///
    /// This operation is zero-copy, which means it keeps the [`Bytes`] returned by underlying
    /// storage services without any extra copy or intensive memory allocations.
    ///
    /// # Notes
    ///
    /// - Buffer length smaller than range means we have reached the end of file.
    pub async fn read(&self, range: impl RangeBounds<u64>) -> Result<Buffer> {
        let start = match range.start_bound().cloned() {
            Bound::Included(start) => start,
            Bound::Excluded(start) => start + 1,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound().cloned() {
            Bound::Included(end) => Some(end + 1),
            Bound::Excluded(end) => Some(end),
            Bound::Unbounded => None,
        };

        // If range is empty, return Ok(0) directly.
        if let Some(end) = end {
            if end <= start {
                return Ok(Buffer::new());
            }
        }

        let mut bufs = Vec::new();
        let mut offset = start;

        loop {
            // TODO: use service preferred io size instead.
            let limit = end.map(|end| end - offset).unwrap_or(4 * 1024 * 1024) as usize;
            let bs = self.inner.read_at_dyn(offset, limit).await?;
            let n = bs.remaining();
            bufs.push(bs);
            if n < limit {
                return Ok(bufs.into_iter().flatten().collect());
            }

            offset += n as u64;
            if Some(offset) == end {
                return Ok(bufs.into_iter().flatten().collect());
            }
        }
    }

    /// Read all data from reader into given [`BufMut`].
    ///
    /// This operation will copy and write bytes into given [`BufMut`]. Allocation happens while
    /// [`BufMut`] doesn't have enough space.
    ///
    /// # Notes
    ///
    /// - Returning length smaller than range means we have reached the end of file.
    pub async fn read_into(
        &self,
        buf: &mut impl BufMut,
        range: impl RangeBounds<u64>,
    ) -> Result<usize> {
        let start = match range.start_bound().cloned() {
            Bound::Included(start) => start,
            Bound::Excluded(start) => start + 1,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound().cloned() {
            Bound::Included(end) => Some(end + 1),
            Bound::Excluded(end) => Some(end),
            Bound::Unbounded => None,
        };

        // If range is empty, return Ok(0) directly.
        if let Some(end) = end {
            if end <= start {
                return Ok(0);
            }
        }

        let mut offset = start;
        let mut read = 0;

        loop {
            // TODO: use service preferred io size instead.
            let limit = end.map(|end| end - offset).unwrap_or(4 * 1024 * 1024) as usize;
            let bs = self.inner.read_at_dyn(offset, limit).await?;
            let n = bs.remaining();
            buf.put(bs);
            read += n as u64;
            if n < limit {
                return Ok(read as _);
            }

            offset += n as u64;
            if Some(offset) == end {
                return Ok(read as _);
            }
        }
    }

    /// Convert reader into [`FuturesIoAsyncReader`] which implements [`futures::AsyncRead`],
    /// [`futures::AsyncSeek`] and [`futures::AsyncBufRead`].
    #[inline]
    pub fn into_futures_io_async_read(self, range: Range<u64>) -> FuturesIoAsyncReader {
        FuturesIoAsyncReader::new(self.inner, range)
    }

    /// Convert reader into [`FuturesBytesStream`] which implements [`futures::Stream`],
    /// [`futures::AsyncSeek`] and [`futures::AsyncBufRead`].
    #[inline]
    pub fn into_futures_bytes_stream(self, range: Range<u64>) -> FuturesBytesStream {
        FuturesBytesStream::new(self.inner, range)
    }
}

pub mod into_futures_async_read {
    use std::io;
    use std::io::SeekFrom;
    use std::ops::Range;
    use std::pin::Pin;
    use std::task::ready;
    use std::task::Context;
    use std::task::Poll;

    use bytes::Buf;
    use futures::AsyncBufRead;
    use futures::AsyncRead;
    use futures::AsyncSeek;

    use crate::raw::*;
    use crate::*;

    /// FuturesAsyncReader is the adapter of [`AsyncRead`], [`AsyncBufRead`] and [`AsyncSeek`]
    /// for [`Reader`].
    ///
    /// Users can use this adapter in cases where they need to use [`AsyncRead`] related trait.
    ///
    /// FuturesAsyncReader also implements [`Unpin`], [`Send`] and [`Sync`]
    pub struct FuturesIoAsyncReader {
        state: State,
        offset: u64,
        size: u64,
        cap: usize,

        cur: u64,
        buf: Buffer,
    }

    enum State {
        Idle(Option<oio::Reader>),
        Fill(BoxedStaticFuture<(oio::Reader, Result<Buffer>)>),
    }

    /// # Safety
    ///
    /// FuturesReader only exposes `&mut self` to the outside world, so it's safe to be `Sync`.
    unsafe impl Sync for State {}

    impl FuturesIoAsyncReader {
        /// NOTE: don't allow users to create FuturesAsyncReader directly.
        #[inline]
        pub(super) fn new(r: oio::Reader, range: Range<u64>) -> Self {
            FuturesIoAsyncReader {
                state: State::Idle(Some(r)),
                offset: range.start,
                size: range.end - range.start,
                // TODO: should use services preferred io size.
                cap: 4 * 1024 * 1024,

                cur: 0,
                buf: Buffer::new(),
            }
        }

        /// Set the capacity of this reader to control the IO size.
        pub fn with_capacity(mut self, cap: usize) -> Self {
            self.cap = cap;
            self
        }
    }

    impl AsyncBufRead for FuturesIoAsyncReader {
        fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
            let this = self.get_mut();
            loop {
                if this.buf.has_remaining() {
                    return Poll::Ready(Ok(this.buf.chunk()));
                }

                match &mut this.state {
                    State::Idle(r) => {
                        // Make sure cur didn't exceed size.
                        if this.cur >= this.size {
                            return Poll::Ready(Ok(&[]));
                        }

                        let r = r.take().expect("reader must be present");
                        let next_offset = this.offset + this.cur;
                        let next_size = (this.size - this.cur).min(this.cap as u64) as usize;
                        let fut = async move {
                            let res = r.read_at_dyn(next_offset, next_size).await;
                            (r, res)
                        };
                        this.state = State::Fill(Box::pin(fut));
                    }
                    State::Fill(fut) => {
                        let (r, res) = ready!(fut.as_mut().poll(cx));
                        this.state = State::Idle(Some(r));
                        this.buf = res?;
                    }
                }
            }
        }

        fn consume(mut self: Pin<&mut Self>, amt: usize) {
            self.buf.advance(amt);
            self.cur += amt as u64;
        }
    }

    impl AsyncRead for FuturesIoAsyncReader {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let bs = ready!(self.as_mut().poll_fill_buf(cx))?;
            let n = bs.len().min(buf.len());
            buf[..n].copy_from_slice(&bs[..n]);
            self.as_mut().consume(n);
            Poll::Ready(Ok(n))
        }
    }

    impl AsyncSeek for FuturesIoAsyncReader {
        fn poll_seek(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            pos: SeekFrom,
        ) -> Poll<io::Result<u64>> {
            let new_pos = match pos {
                SeekFrom::Start(pos) => pos as i64,
                SeekFrom::End(pos) => self.size as i64 + pos,
                SeekFrom::Current(pos) => self.cur as i64 + pos,
            };

            if new_pos < 0 {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid seek to a negative position",
                )));
            }

            let new_pos = new_pos as u64;

            if (self.cur..self.cur + self.buf.remaining() as u64).contains(&new_pos) {
                let cnt = new_pos - self.cur;
                self.buf.advance(cnt as _);
            } else {
                self.buf = Buffer::new()
            }

            self.cur = new_pos;
            Poll::Ready(Ok(self.cur))
        }
    }
}

pub mod into_futures_stream {
    use std::io;
    use std::ops::Range;
    use std::pin::Pin;
    use std::task::ready;
    use std::task::Context;
    use std::task::Poll;

    use bytes::Buf;
    use bytes::Bytes;
    use futures::Stream;

    use crate::raw::*;
    use crate::*;

    /// FuturesStream is the adapter of [`Stream`] for [`Reader`].
    ///
    /// Users can use this adapter in cases where they need to use [`Stream`] trait.
    ///
    /// FuturesStream also implements [`Unpin`], [`Send`] and [`Sync`].
    pub struct FuturesBytesStream {
        state: State,
        offset: u64,
        size: u64,
        cap: usize,

        cur: u64,
    }

    enum State {
        Idle(Option<oio::Reader>),
        Next(BoxedStaticFuture<(oio::Reader, Result<Buffer>)>),
    }

    /// # Safety
    ///
    /// FuturesReader only exposes `&mut self` to the outside world, so it's safe to be `Sync`.
    unsafe impl Sync for State {}

    impl FuturesBytesStream {
        /// NOTE: don't allow users to create FuturesStream directly.
        #[inline]
        pub(crate) fn new(r: oio::Reader, range: Range<u64>) -> Self {
            FuturesBytesStream {
                state: State::Idle(Some(r)),
                offset: range.start,
                size: range.end - range.start,
                // TODO: should use services preferred io size.
                cap: 4 * 1024 * 1024,

                cur: 0,
            }
        }

        /// Set the capacity of this reader to control the IO size.
        pub fn with_capacity(mut self, cap: usize) -> Self {
            self.cap = cap;
            self
        }
    }

    impl Stream for FuturesBytesStream {
        type Item = io::Result<Bytes>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let this = self.get_mut();

            loop {
                match &mut this.state {
                    State::Idle(r) => {
                        // Make sure cur didn't exceed size.
                        if this.cur >= this.size {
                            return Poll::Ready(None);
                        }

                        let r = r.take().expect("reader must be present");
                        let next_offset = this.offset + this.cur;
                        let next_size = (this.size - this.cur).min(this.cap as u64) as usize;
                        let fut = async move {
                            let res = r.read_at_dyn(next_offset, next_size).await;
                            (r, res)
                        };
                        this.state = State::Next(Box::pin(fut));
                    }
                    State::Next(fut) => {
                        let (r, res) = ready!(fut.as_mut().poll(cx));
                        this.state = State::Idle(Some(r));
                        return match res {
                            Ok(buf) if !buf.has_remaining() => Poll::Ready(None),
                            Ok(mut buf) => {
                                this.cur += buf.remaining() as u64;
                                Poll::Ready(Some(Ok(buf.copy_to_bytes(buf.remaining()))))
                            }
                            Err(err) => Poll::Ready(Some(Err(format_std_io_error(err)))),
                        };
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::ThreadRng;
    use rand::Rng;
    use rand::RngCore;

    use crate::services;
    use crate::Operator;

    fn gen_random_bytes() -> Vec<u8> {
        let mut rng = ThreadRng::default();
        // Generate size between 1B..16MB.
        let size = rng.gen_range(1..16 * 1024 * 1024);
        let mut content = vec![0; size];
        rng.fill_bytes(&mut content);
        content
    }

    #[tokio::test]
    async fn test_reader_read() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader(path).await.unwrap();
        let buf = reader.read(..).await.expect("read to end must succeed");

        assert_eq!(buf.to_bytes(), content);
    }

    #[tokio::test]
    async fn test_reader_read_into() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader(path).await.unwrap();
        let mut buf = Vec::new();
        reader
            .read_into(&mut buf, ..)
            .await
            .expect("read to end must succeed");

        assert_eq!(buf, content);
    }
}
