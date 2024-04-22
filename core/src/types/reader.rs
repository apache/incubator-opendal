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

use std::ops::Range;
use std::ops::RangeBounds;

use bytes::BufMut;
use futures::stream;
use futures::StreamExt;
use futures::TryStreamExt;

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
#[derive(Clone)]
pub struct Reader {
    inner: oio::Reader,
    options: OpReader,
}

impl Reader {
    /// Create a new reader.
    ///
    /// Create will use internal information to decide the most suitable
    /// implementation for users.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) async fn create(
        acc: FusedAccessor,
        path: &str,
        args: OpRead,
        options: OpReader,
    ) -> Result<Self> {
        let (_, r) = acc.read(path, args).await?;

        Ok(Reader { inner: r, options })
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
        let bufs: Vec<_> = self.clone().into_stream(range).try_collect().await?;
        Ok(bufs.into_iter().flatten().collect())
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
        let mut stream = self.clone().into_stream(range);

        let mut read = 0;
        loop {
            let Some(bs) = stream.try_next().await? else {
                return Ok(read);
            };
            read += bs.len();
            buf.put(bs);
        }
    }

    /// Fetch specific ranges from reader.
    ///
    /// This operation try to merge given ranges into a list of
    /// non-overlapping ranges. Users may also specify a `gap` to merge
    /// close ranges.
    ///
    /// The returning `Buffer` may share the same underlying memory without
    /// any extra copy.
    pub async fn fetch(&self, ranges: Vec<Range<u64>>) -> Result<Vec<Buffer>> {
        let merged_ranges = self.merge_ranges(ranges.clone());

        let merged_bufs: Vec<_> =
            stream::iter(merged_ranges.clone().into_iter().map(|v| self.read(v)))
                .buffered(self.options.concurrent())
                .try_collect()
                .await?;

        let mut bufs = Vec::with_capacity(ranges.len());
        for range in ranges {
            let idx = merged_ranges.partition_point(|v| v.start <= range.start) - 1;
            let start = range.start - merged_ranges[idx].start;
            let end = range.end - merged_ranges[idx].start;
            bufs.push(merged_bufs[idx].slice(start as usize..end as usize));
        }

        Ok(bufs)
    }

    /// Merge given ranges into a list of non-overlapping ranges.
    fn merge_ranges(&self, mut ranges: Vec<Range<u64>>) -> Vec<Range<u64>> {
        let gap = self.options.gap().unwrap_or(1024 * 1024) as u64;
        // We don't care about the order of range with same start, they
        // will be merged in the next step.
        ranges.sort_unstable_by(|a, b| a.start.cmp(&b.start));

        // We know that this vector will have at most element
        let mut merged = Vec::with_capacity(ranges.len());
        let mut cur = ranges[0].clone();

        for range in ranges.into_iter().skip(1) {
            if range.start <= cur.end + gap {
                // There is an overlap or the gap is small enough to merge
                cur.end = cur.end.max(range.end);
            } else {
                // No overlap and the gap is too large, push the current range to the list and start a new one
                merged.push(cur);
                cur = range;
            }
        }

        // Push the last range
        merged.push(cur);

        merged
    }

    /// Create a buffer stream to read specific range from given reader.
    ///
    /// # Notes
    ///
    /// This API can be public but we are not sure if it's useful for users.
    /// And the name `BufferStream` is not good enough to expose to users.
    /// Let's keep it inside for now.
    fn into_stream(self, range: impl RangeBounds<u64>) -> into_stream::BufferStream {
        into_stream::BufferStream::new(self.inner, self.options, range)
    }

    /// Convert reader into [`FuturesAsyncReader`] which implements [`futures::AsyncRead`],
    /// [`futures::AsyncSeek`] and [`futures::AsyncBufRead`].
    ///
    /// # TODO
    ///
    /// Extend this API to accept `impl RangeBounds`.
    #[inline]
    pub fn into_futures_async_read(self, range: Range<u64>) -> FuturesAsyncReader {
        FuturesAsyncReader::new(self.inner, self.options, range)
    }

    /// Convert reader into [`FuturesBytesStream`] which implements [`futures::Stream`].
    #[inline]
    pub fn into_bytes_stream(self, range: impl RangeBounds<u64>) -> FuturesBytesStream {
        FuturesBytesStream::new(self.inner, self.options, range)
    }
}

pub mod into_stream {
    use std::pin::Pin;
    use std::sync::atomic::Ordering;
    use std::task::{Context, Poll};
    use std::{
        ops::{Bound, RangeBounds},
        sync::{atomic::AtomicBool, Arc},
    };

    use futures::stream::{self, Buffered, Iter};
    use futures::{Stream, StreamExt};

    use crate::raw::*;
    use crate::*;

    /// ReadFutureIterator is an iterator that returns future of [`Buffer`] from [`Reader`].
    struct ReadFutureIterator {
        r: oio::Reader,
        chunk: Option<usize>,

        offset: u64,
        end: Option<u64>,
        finished: Arc<AtomicBool>,
    }

    impl ReadFutureIterator {
        #[inline]
        fn new(r: oio::Reader, chunk: Option<usize>, range: impl RangeBounds<u64>) -> Self {
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

            ReadFutureIterator {
                r,
                chunk,
                offset: start,
                end,
                finished: Arc::default(),
            }
        }
    }

    impl Iterator for ReadFutureIterator {
        type Item = BoxedFuture<'static, Result<Buffer>>;

        fn next(&mut self) -> Option<Self::Item> {
            if self.offset >= self.end.unwrap_or(u64::MAX) {
                return None;
            }
            if self.finished.load(Ordering::Relaxed) {
                return None;
            }

            let offset = self.offset;
            // TODO: replace with services preferred chunk size.
            let chunk = self.chunk.unwrap_or(4 * 1024 * 1024);
            let limit = self
                .end
                .map(|end| ((end - self.offset) as usize).min(chunk))
                .unwrap_or(chunk);
            let finished = self.finished.clone();
            let r = self.r.clone();

            // Update self.offset before building future.
            self.offset += limit as u64;
            let fut = async move {
                let buf = r.read_at_dyn(offset, limit).await?;
                if buf.len() < limit || limit == 0 {
                    // Update finished marked if buf is less than limit.
                    finished.store(true, Ordering::Relaxed);
                }
                Ok(buf)
            };

            Some(Box::pin(fut))
        }
    }

    /// BufferStream is a stream that returns [`Buffer`] from [`Reader`].
    ///
    /// This stream will use concurrent read to fetch data from underlying storage.
    ///
    /// # Notes
    ///
    /// BufferStream uses `Buffered<Iter<ReadFutureIterator>>` internally,
    /// but we want to hide those details from users.
    pub struct BufferStream(Buffered<Iter<ReadFutureIterator>>);

    impl BufferStream {
        /// Create a new buffer stream from given reader.
        #[inline]
        pub fn new(r: oio::Reader, options: OpReader, range: impl RangeBounds<u64>) -> Self {
            let iter = ReadFutureIterator::new(r, options.chunk(), range);
            let stream = stream::iter(iter).buffered(options.concurrent());

            BufferStream(stream)
        }
    }

    impl Stream for BufferStream {
        type Item = Result<Buffer>;

        #[inline]
        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            self.0.poll_next_unpin(cx)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::raw::MaybeSend;

        trait AssertTrait: Unpin + MaybeSend + 'static {}
        impl AssertTrait for BufferStream {}
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
    use futures::StreamExt;

    use crate::raw::*;
    use crate::*;

    use super::into_stream::BufferStream;

    /// FuturesAsyncReader is the adapter of [`AsyncRead`], [`AsyncBufRead`] and [`AsyncSeek`]
    /// for [`Reader`].
    ///
    /// Users can use this adapter in cases where they need to use [`AsyncRead`] related trait.
    ///
    /// FuturesAsyncReader also implements [`Unpin`], [`Send`] and [`Sync`]
    pub struct FuturesAsyncReader {
        r: oio::Reader,
        options: OpReader,

        stream: BufferStream,
        buf: Buffer,
        start: u64,
        end: u64,
        pos: u64,
    }

    impl FuturesAsyncReader {
        /// NOTE: don't allow users to create FuturesAsyncReader directly.
        #[inline]
        pub(super) fn new(r: oio::Reader, options: OpReader, range: Range<u64>) -> Self {
            let (start, end) = (range.start, range.end);
            let stream = BufferStream::new(r.clone(), options.clone(), range);

            FuturesAsyncReader {
                r,
                options,
                stream,
                buf: Buffer::new(),
                start,
                end,
                pos: 0,
            }
        }
    }

    impl AsyncBufRead for FuturesAsyncReader {
        fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
            let this = self.get_mut();
            loop {
                if this.buf.has_remaining() {
                    return Poll::Ready(Ok(this.buf.chunk()));
                }

                this.buf = match ready!(this.stream.poll_next_unpin(cx)) {
                    Some(Ok(buf)) => buf,
                    Some(Err(err)) => return Poll::Ready(Err(format_std_io_error(err))),
                    None => return Poll::Ready(Ok(&[])),
                };
            }
        }

        fn consume(mut self: Pin<&mut Self>, amt: usize) {
            self.buf.advance(amt);
            // Make sure buf has been dropped before starting new request.
            // Otherwise, we will hold those bytes in memory until next
            // buffer reaching.
            if self.buf.is_empty() {
                self.buf = Buffer::new();
            }
            self.pos += amt as u64;
        }
    }

    /// TODO: implement vectored read.
    impl AsyncRead for FuturesAsyncReader {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let this = self.get_mut();

            loop {
                if this.buf.remaining() > 0 {
                    let size = this.buf.remaining().min(buf.len());
                    this.buf.copy_to_slice(&mut buf[..size]);
                    this.pos += size as u64;
                    return Poll::Ready(Ok(size));
                }

                this.buf = match ready!(this.stream.poll_next_unpin(cx)) {
                    Some(Ok(buf)) => buf,
                    Some(Err(err)) => return Poll::Ready(Err(format_std_io_error(err))),
                    None => return Poll::Ready(Ok(0)),
                };
            }
        }
    }

    impl AsyncSeek for FuturesAsyncReader {
        fn poll_seek(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            pos: SeekFrom,
        ) -> Poll<io::Result<u64>> {
            let new_pos = match pos {
                SeekFrom::Start(pos) => pos as i64,
                SeekFrom::End(pos) => self.end as i64 - self.start as i64 + pos,
                SeekFrom::Current(pos) => self.pos as i64 + pos,
            };

            // Check if new_pos is negative.
            if new_pos < 0 {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid seek to a negative position",
                )));
            }

            let new_pos = new_pos as u64;

            if (self.pos..self.pos + self.buf.remaining() as u64).contains(&new_pos) {
                let cnt = new_pos - self.pos;
                self.buf.advance(cnt as _);
            } else {
                self.buf = Buffer::new();
                self.stream =
                    BufferStream::new(self.r.clone(), self.options.clone(), new_pos..self.end);
            }

            self.pos = new_pos;
            Poll::Ready(Ok(self.pos))
        }
    }
}

pub mod into_futures_stream {
    use std::io;
    use std::ops::RangeBounds;
    use std::pin::Pin;
    use std::task::ready;
    use std::task::Context;
    use std::task::Poll;

    use bytes::Bytes;
    use futures::Stream;
    use futures::StreamExt;

    use super::into_stream::BufferStream;
    use crate::raw::*;
    use crate::*;

    /// FuturesStream is the adapter of [`Stream`] for [`Reader`].
    ///
    /// Users can use this adapter in cases where they need to use [`Stream`] trait.
    ///
    /// FuturesStream also implements [`Unpin`], [`Send`] and [`Sync`].
    pub struct FuturesBytesStream {
        stream: BufferStream,
        buf: Buffer,
    }

    impl FuturesBytesStream {
        /// NOTE: don't allow users to create FuturesStream directly.
        #[inline]
        pub(crate) fn new(r: oio::Reader, options: OpReader, range: impl RangeBounds<u64>) -> Self {
            let stream = BufferStream::new(r, options, range);

            FuturesBytesStream {
                stream,
                buf: Buffer::new(),
            }
        }
    }

    impl Stream for FuturesBytesStream {
        type Item = io::Result<Bytes>;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let this = self.get_mut();

            loop {
                // Consume current buffer
                if let Some(bs) = Iterator::next(&mut this.buf) {
                    return Poll::Ready(Some(Ok(bs)));
                }

                this.buf = match ready!(this.stream.poll_next_unpin(cx)) {
                    Some(Ok(buf)) => buf,
                    Some(Err(err)) => return Poll::Ready(Some(Err(format_std_io_error(err)))),
                    None => return Poll::Ready(None),
                };
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

    fn gen_fixed_bytes(size: usize) -> Vec<u8> {
        let mut rng = ThreadRng::default();
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
    async fn test_reader_read_with_chunk() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader_with(path).chunk(16).await.unwrap();
        let buf = reader.read(..).await.expect("read to end must succeed");

        assert_eq!(buf.to_bytes(), content);
    }

    #[tokio::test]
    async fn test_reader_read_with_concurrent() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op
            .reader_with(path)
            .chunk(128)
            .concurrent(16)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn test_merge_ranges() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader_with(path).gap(1).await.unwrap();

        let ranges = vec![0..10, 10..20, 21..30, 40..50, 40..60, 45..59];
        let merged = reader.merge_ranges(ranges.clone());
        assert_eq!(merged, vec![0..30, 40..60]);
    }

    #[tokio::test]
    async fn test_fetch() {
        let op = Operator::new(services::Memory::default()).unwrap().finish();
        let path = "test_file";

        let content = gen_fixed_bytes(1024);
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader_with(path).gap(1).await.unwrap();

        let ranges = vec![
            0..10,
            40..50,
            45..59,
            10..20,
            21..30,
            40..50,
            40..60,
            45..59,
        ];
        let merged = reader
            .fetch(ranges.clone())
            .await
            .expect("fetch must succeed");

        for (i, range) in ranges.iter().enumerate() {
            assert_eq!(
                merged[i].to_bytes(),
                content[range.start as usize..range.end as usize]
            );
        }
    }
}
