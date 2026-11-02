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
use std::sync::Arc;

use bytes::BufMut;
use futures::TryStreamExt;

use crate::raw::ConcurrentTasks;
use crate::raw::RpRead;
use crate::raw::oio::Read as _;
use crate::types::BytesRange;
use crate::*;

/// Reader is designed to read data from given path in an asynchronous
/// manner.
///
/// # Usage
///
/// [`Reader`] provides multiple ways to read data from given reader.
///
/// `Reader` implements `Clone` so you can clone it and store in place where ever you want.
///
/// ## Direct
///
/// [`Reader`] provides public API including [`Reader::read`]. You can use those APIs directly without extra copy.
///
/// ```
/// use opendal_core::Operator;
/// use opendal_core::Result;
///
/// async fn test(op: Operator) -> Result<()> {
///     let r = op.reader("path/to/file").await?;
///     let bs = r.read(0..1024).await?;
///     Ok(())
/// }
/// ```
///
/// ## Read like `Stream`
///
/// ```
/// use anyhow::Result;
/// use bytes::Bytes;
/// use futures::TryStreamExt;
/// use opendal_core::Operator;
///
/// async fn test(op: Operator) -> Result<()> {
///     let s = op
///         .reader("path/to/file")
///         .await?
///         .into_bytes_stream(1024..2048)
///         .await?;
///     let bs: Vec<Bytes> = s.try_collect().await?;
///     Ok(())
/// }
/// ```
///
/// ## Read like `AsyncRead` and `AsyncBufRead`
///
/// ```
/// use anyhow::Result;
/// use bytes::Bytes;
/// use futures::AsyncReadExt;
/// use opendal_core::Operator;
///
/// async fn test(op: Operator) -> Result<()> {
///     let mut r = op
///         .reader("path/to/file")
///         .await?
///         .into_futures_async_read(1024..2048)
///         .await?;
///     let mut bs = vec![];
///     let n = r.read_to_end(&mut bs).await?;
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Reader {
    ctx: Arc<ReadContext>,
}

struct FetchReadInput {
    ctx: Arc<ReadContext>,
    range_idx: usize,
    range: BytesRange,
}

struct FetchReadOutput {
    range_idx: usize,
    offset: u64,
    size: u64,
    rp: RpRead,
    buffer: Buffer,
}

impl Reader {
    /// Create a new reader.
    ///
    /// Create will use internal information to decide the most suitable
    /// implementation for users.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) fn new(ctx: ReadContext) -> Self {
        Reader { ctx: Arc::new(ctx) }
    }

    pub(crate) async fn parse_into_range(&self, range: BytesRange) -> Result<Range<u64>> {
        self.ctx.parse_into_range(range).await
    }

    /// Get complete object metadata observed by this reader.
    ///
    /// This method doesn't perform I/O. It returns `None` if no read has
    /// observed complete object metadata yet, or if the underlying service
    /// doesn't return metadata while opening read operations.
    pub fn metadata(&self) -> Option<&Metadata> {
        self.ctx.metadata()
    }

    /// Read give range from reader into [`Buffer`].
    ///
    /// This operation is zero-copy, which means it keeps the [`bytes::Bytes`] returned by underlying
    /// storage services without any extra copy or intensive memory allocations.
    pub async fn read(&self, range: impl Into<BytesRange>) -> Result<Buffer> {
        let bufs: Vec<_> = self.clone().into_stream(range).await?.try_collect().await?;
        Ok(bufs.into_iter().flatten().collect())
    }

    /// Read all data from reader into given [`BufMut`].
    ///
    /// This operation will copy and write bytes into given [`BufMut`]. Allocation happens while
    /// [`BufMut`] doesn't have enough space.
    pub async fn read_into(
        &self,
        buf: &mut impl BufMut,
        range: impl Into<BytesRange>,
    ) -> Result<usize> {
        let mut stream = self.clone().into_stream(range).await?;

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
    /// This operation tries to merge given ranges into a list of
    /// non-overlapping ranges. Users may also specify a `gap` to merge
    /// close ranges. Merged ranges will be split by `chunk`, then executed
    /// with `concurrent` and `prefetch`.
    /// Set `gap` to `0` to avoid merging ranges separated by any bytes.
    /// Overlapping or adjacent ranges are still merged.
    ///
    /// The returning `Buffer` may share the same underlying memory without
    /// any extra copy.
    pub async fn fetch(&self, ranges: Vec<Range<u64>>) -> Result<Vec<Buffer>> {
        if ranges.is_empty() {
            return Ok(vec![]);
        }

        let mut bufs = vec![Buffer::new(); ranges.len()];
        let ranges = ranges
            .into_iter()
            .enumerate()
            .filter(|(_, range)| range.start < range.end)
            .collect::<Vec<_>>();
        if ranges.is_empty() {
            return Ok(bufs);
        }

        let merged_ranges =
            self.merge_ranges(ranges.iter().map(|(_, range)| range.clone()).collect());
        let merged_bufs = self.fetch_merged_ranges(&merged_ranges).await?;

        for (output_idx, range) in ranges {
            let idx = merged_ranges.partition_point(|v| v.start <= range.start) - 1;
            let start = range.start - merged_ranges[idx].start;
            let end = range.end - merged_ranges[idx].start;
            bufs[output_idx] = merged_bufs[idx].slice(start as usize..end as usize);
        }

        Ok(bufs)
    }

    async fn fetch_merged_ranges(&self, ranges: &[Range<u64>]) -> Result<Vec<Buffer>> {
        let inputs = self.plan_fetch_reads(ranges);
        let mut parts = (0..ranges.len())
            .map(|_| Vec::new())
            .collect::<Vec<Vec<(u64, Buffer)>>>();
        let mut tasks = ConcurrentTasks::new(
            self.ctx.context().executor().clone(),
            self.ctx.options().concurrent(),
            self.ctx.options().prefetch(),
            |input: FetchReadInput| {
                Box::pin(async move {
                    let ctx = input.ctx.clone();
                    let range = input.range;
                    let range_idx = input.range_idx;

                    let offset = range.offset();
                    let size = range
                        .size()
                        .expect("fetch planner must only create bounded ranges");
                    let result = match ctx.reader().read(range).await {
                        Ok((rp, buffer)) => Ok(FetchReadOutput {
                            range_idx,
                            offset,
                            size,
                            rp,
                            buffer,
                        }),
                        Err(err) => Err(err),
                    };

                    (input, result)
                })
            },
        );

        for input in inputs {
            tasks.execute(input).await?;
            while tasks.has_result() {
                let output = tasks
                    .next()
                    .await
                    .transpose()?
                    .expect("task result must exist");
                self.push_fetch_output(&mut parts, output)?;
            }
        }

        while let Some(output) = tasks.next().await.transpose()? {
            self.push_fetch_output(&mut parts, output)?;
        }

        Ok(parts
            .into_iter()
            .map(|mut v| {
                v.sort_unstable_by_key(|(offset, _)| *offset);
                v.into_iter().flat_map(|(_, buffer)| buffer).collect()
            })
            .collect())
    }

    fn plan_fetch_reads(&self, ranges: &[Range<u64>]) -> Vec<FetchReadInput> {
        let chunk = self.ctx.options().chunk().map(|v| v as u64);
        let mut inputs = Vec::with_capacity(ranges.len());

        for (range_idx, range) in ranges.iter().enumerate() {
            let mut offset = range.start;
            while offset < range.end {
                let remaining = range.end - offset;
                let size = chunk.map_or(remaining, |v| remaining.min(v));
                inputs.push(FetchReadInput {
                    ctx: self.ctx.clone(),
                    range_idx,
                    range: BytesRange::new(offset, Some(size)),
                });
                offset += size;
            }
        }

        inputs
    }

    fn push_fetch_output(
        &self,
        parts: &mut [Vec<(u64, Buffer)>],
        output: FetchReadOutput,
    ) -> Result<()> {
        if output.buffer.len() as u64 != output.size {
            return Err(
                Error::new(ErrorKind::Unexpected, "reader got unexpected data size")
                    .with_context("expect", output.size)
                    .with_context("actual", output.buffer.len() as u64),
            );
        }

        self.ctx.observe_read_response(output.rp);
        parts[output.range_idx].push((output.offset, output.buffer));
        Ok(())
    }

    /// Merge given ranges into a list of non-overlapping ranges.
    fn merge_ranges(&self, mut ranges: Vec<Range<u64>>) -> Vec<Range<u64>> {
        let gap = self.ctx.options().gap().unwrap_or(1024 * 1024) as u64;
        // We don't care about the order of range with same start, they
        // will be merged in the next step.
        ranges.sort_unstable_by_key(|a| a.start);

        // We know that this vector will have at most element
        let mut merged = Vec::with_capacity(ranges.len());
        let mut cur = ranges[0].clone();

        for range in ranges.into_iter().skip(1) {
            if range.start <= cur.end.saturating_add(gap) {
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
    /// BufferStream is a zero-cost abstraction. It doesn't involve extra copy of data.
    /// It will return underlying [`Buffer`] directly.
    ///
    /// The [`Buffer`] this stream yields can be seen as an iterator of
    /// [`bytes::Bytes`].
    ///
    /// # Inputs
    ///
    /// - `range`: The range of data to read. range like `..` it will read all data from reader.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```
    /// use std::io;
    ///
    /// use bytes::Bytes;
    /// use futures::TryStreamExt;
    /// use opendal_core::Buffer;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut s = op
    ///         .reader("hello.txt")
    ///         .await?
    ///         .into_stream(1024..2048)
    ///         .await?;
    ///
    ///     let bs: Vec<Buffer> = s.try_collect().await?;
    ///     // We can use those buffer as bytes if we want.
    ///     let bytes_vec: Vec<Bytes> = bs.clone().into_iter().flatten().collect();
    ///     // Or we can merge them into a single [`Buffer`] and later use it as [`bytes::Buf`].
    ///     let new_buffer: Buffer = bs.into_iter().flatten().collect::<Buffer>();
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Concurrent Read
    ///
    /// The following example reads data in 256B chunks with 8 concurrent.
    ///
    /// ```
    /// use std::io;
    ///
    /// use bytes::Bytes;
    /// use futures::TryStreamExt;
    /// use opendal_core::Buffer;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let s = op
    ///         .reader_with("hello.txt")
    ///         .concurrent(8)
    ///         .chunk(256)
    ///         .await?
    ///         .into_stream(1024..2048)
    ///         .await?;
    ///
    ///     // Every buffer except the last one in the stream will be 256B.
    ///     let bs: Vec<Buffer> = s.try_collect().await?;
    ///     // We can use those buffer as bytes if we want.
    ///     let bytes_vec: Vec<Bytes> = bs.clone().into_iter().flatten().collect();
    ///     // Or we can merge them into a single [`Buffer`] and later use it as [`bytes::Buf`].
    ///     let new_buffer: Buffer = bs.into_iter().flatten().collect::<Buffer>();
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn into_stream(self, range: impl Into<BytesRange>) -> Result<BufferStream> {
        BufferStream::create(self.ctx, range).await
    }

    /// Convert reader into [`FuturesAsyncReader`] which implements [`futures::AsyncRead`],
    /// [`futures::AsyncSeek`] and [`futures::AsyncBufRead`].
    ///
    /// Unbounded ranges resolve the object length only for operations that require it, such as
    /// seeking relative to the end. Seeking from the start or current position can move beyond
    /// the end without resolving the length, and subsequent reads return EOF. Explicit bounded
    /// ranges continue to reject seeks beyond their logical end.
    ///
    /// # Notes
    ///
    /// FuturesAsyncReader is not a zero-cost abstraction. The underlying reader
    /// returns an owned [`Buffer`], which involves an extra copy operation.
    ///
    /// # Inputs
    ///
    /// - `range`: The range of data to read. range like `..` it will read all data from reader.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```
    /// use std::io;
    ///
    /// use futures::io::AsyncReadExt;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut r = op
    ///         .reader("hello.txt")
    ///         .await?
    ///         .into_futures_async_read(1024..2048)
    ///         .await?;
    ///     let mut bs = Vec::new();
    ///     r.read_to_end(&mut bs).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Concurrent Read
    ///
    /// The following example reads data in 256B chunks with 8 concurrent.
    ///
    /// ```
    /// use std::io;
    ///
    /// use futures::io::AsyncReadExt;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut r = op
    ///         .reader_with("hello.txt")
    ///         .concurrent(8)
    ///         .chunk(256)
    ///         .await?
    ///         .into_futures_async_read(1024..2048)
    ///         .await?;
    ///     let mut bs = Vec::new();
    ///     r.read_to_end(&mut bs).await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub async fn into_futures_async_read(
        self,
        range: impl Into<BytesRange>,
    ) -> Result<FuturesAsyncReader> {
        FuturesAsyncReader::new(self.ctx, range).await
    }

    /// Convert reader into [`FuturesBytesStream`] which implements [`futures::Stream`].
    ///
    /// # Inputs
    ///
    /// - `range`: The range of data to read. range like `..` it will read all data from reader.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```
    /// use std::io;
    ///
    /// use bytes::Bytes;
    /// use futures::TryStreamExt;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut s = op
    ///         .reader("hello.txt")
    ///         .await?
    ///         .into_bytes_stream(1024..2048)
    ///         .await?;
    ///     let bs: Vec<Bytes> = s.try_collect().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Concurrent Read
    ///
    /// The following example reads data in 256B chunks with 8 concurrent.
    ///
    /// ```
    /// use std::io;
    ///
    /// use bytes::Bytes;
    /// use futures::TryStreamExt;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut s = op
    ///         .reader_with("hello.txt")
    ///         .concurrent(8)
    ///         .chunk(256)
    ///         .await?
    ///         .into_bytes_stream(1024..2048)
    ///         .await?;
    ///     let bs: Vec<Bytes> = s.try_collect().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub async fn into_bytes_stream(
        self,
        range: impl Into<BytesRange>,
    ) -> Result<FuturesBytesStream> {
        FuturesBytesStream::new(self.ctx, range).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    use bytes::Bytes;
    use rand::{Rng, RngExt};

    use super::*;
    use crate::Operator;
    use crate::raw::*;
    use crate::services;
    use crate::*;

    fn new_read_context(
        ctx: OperationContext,
        srv: Servicer,
        path: &str,
        options: crate::raw::OpReader,
    ) -> crate::Result<ReadContext> {
        let args = crate::raw::OpRead::new();
        let reader = srv.read(&ctx, path, args.clone())?;
        Ok(ReadContext::new(
            ctx,
            srv,
            path.to_string(),
            args,
            options,
            reader,
        ))
    }

    struct MockRangeReader {
        content: Bytes,
        ranges: Arc<Mutex<Vec<BytesRange>>>,
    }

    impl MockRangeReader {
        fn new(content: Bytes, ranges: Arc<Mutex<Vec<BytesRange>>>) -> Self {
            Self { content, ranges }
        }
    }

    impl oio::Read for MockRangeReader {
        async fn open(&self, _: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
            Err(Error::new(ErrorKind::Unsupported, "open is not supported"))
        }

        async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
            let size = range
                .size()
                .ok_or_else(|| Error::new(ErrorKind::Unexpected, "range must be bounded"))?;
            if size == 0 {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "zero-size raw read must not be called",
                ));
            }

            self.ranges.lock().unwrap().push(range);

            let start = range.offset() as usize;
            let end = start + size as usize;
            if range.offset() == 0 && range.size() == Some(4) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            if end > self.content.len() {
                return Err(Error::new(
                    ErrorKind::RangeNotSatisfied,
                    "range exceeds content length",
                ));
            }

            Ok((
                RpRead::new({
                    let metadata = MetadataBuilder::file(self.content.len() as u64);
                    metadata.build()
                }),
                Buffer::from(self.content.slice(start..end)),
            ))
        }
    }

    fn new_mock_reader(
        content: Bytes,
        options: crate::raw::OpReader,
        ranges: Arc<Mutex<Vec<BytesRange>>>,
    ) -> Reader {
        let op = Operator::new(services::Memory::default()).unwrap();
        Reader::new(ReadContext::new(
            op.context().clone(),
            op.service().clone(),
            "test_file".to_string(),
            OpRead::new(),
            options,
            Box::new(MockRangeReader::new(content, ranges)),
        ))
    }

    #[tokio::test]
    async fn test_trait() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        op.write(
            "test",
            Buffer::from(vec![Bytes::from("Hello"), Bytes::from("World")]),
        )
        .await?;

        let ctx = op.context().clone();
        let srv = op.service().clone();
        let ctx = new_read_context(ctx, srv, "test", OpReader::new())?;

        let _: Box<dyn Unpin + MaybeSend + Sync + 'static> = Box::new(Reader::new(ctx));

        Ok(())
    }

    #[tokio::test]
    async fn test_reader_metadata_after_read() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        op.write("test", Buffer::from("HelloWorld")).await?;

        let reader = op.reader("test").await?;
        assert_eq!(reader.metadata(), None);

        let buf = reader.read(4..8).await?;
        assert_eq!(&buf.to_vec(), b"oWor");

        let meta = reader.metadata().expect("metadata must be observed");
        assert_eq!(meta.content_length(), 10);

        Ok(())
    }

    #[tokio::test]
    async fn test_stream_metadata_updates_reader_metadata() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        op.write("test", Buffer::from("HelloWorld")).await?;

        let reader = op.reader("test").await?;
        let mut stream = reader.clone().into_stream(4..8).await?;

        let stream_meta = stream.metadata().await?;
        assert_eq!(stream_meta.content_length(), 10);

        let bufs: Vec<_> = stream.try_collect().await?;
        let buf: Buffer = bufs.into_iter().flatten().collect();
        assert_eq!(&buf.to_vec(), b"oWor");

        let reader_meta = reader.metadata().expect("reader metadata must be observed");
        assert_eq!(reader_meta.content_length(), 10);

        Ok(())
    }

    #[tokio::test]
    async fn test_chunked_stream_metadata_updates_reader_metadata() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        op.write("test", Buffer::from("HelloWorld")).await?;

        let reader = op.reader_with("test").chunk(2).await?;
        let mut stream = reader.clone().into_stream(4..8).await?;

        let stream_meta = stream.metadata().await?;
        assert_eq!(stream_meta.content_length(), 10);

        let bufs: Vec<_> = stream.try_collect().await?;
        let buf: Buffer = bufs.into_iter().flatten().collect();
        assert_eq!(&buf.to_vec(), b"oWor");

        let reader_meta = reader.metadata().expect("reader metadata must be observed");
        assert_eq!(reader_meta.content_length(), 10);

        Ok(())
    }

    fn gen_random_bytes() -> Vec<u8> {
        let mut rng = rand::rng();
        // Generate size between 1B..16MB.
        let size = rng.random_range(1..16 * 1024 * 1024);
        let mut content = vec![0; size];
        rng.fill_bytes(&mut content);
        content
    }

    fn gen_fixed_bytes(size: usize) -> Vec<u8> {
        let mut rng = rand::rng();
        let mut content = vec![0; size];
        rng.fill_bytes(&mut content);
        content
    }

    #[tokio::test]
    async fn test_reader_read() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader(path).await.unwrap();
        let buf = reader.read(..).await.expect("read to end must succeed");

        assert_eq!(buf.to_bytes(), content);
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_read_with_chunk() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader_with(path).chunk(16).await.unwrap();
        let buf = reader.read(..).await.expect("read to end must succeed");

        assert_eq!(buf.to_bytes(), content);
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_read_with_concurrent() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
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
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_read_suffix() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let path = "test_file";
        let content = Bytes::from_static(b"HelloWorld");
        op.write(path, Buffer::from(content.clone())).await?;

        let reader = op.reader(path).await?;
        let buf = reader.read(BytesRange::suffix(4)).await?;

        assert_eq!(buf.to_bytes(), b"orld".as_slice());
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_read_suffix_larger_than_content() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let path = "test_file";
        let content = Bytes::from_static(b"HelloWorld");
        op.write(path, Buffer::from(content.clone())).await?;

        let reader = op.reader(path).await?;
        let buf = reader.read(BytesRange::suffix(20)).await?;

        assert_eq!(buf.to_bytes(), content);
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_read_suffix_zero() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let path = "test_file";
        op.write(path, Buffer::from("HelloWorld")).await?;

        let reader = op.reader(path).await?;
        let buf = reader.read(BytesRange::suffix(0)).await?;

        assert!(buf.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_read_suffix_empty_content() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let path = "test_file";
        op.write(path, Buffer::new()).await?;

        let reader = op.reader(path).await?;
        let buf = reader.read(BytesRange::suffix(4)).await?;

        assert!(buf.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_read_suffix_with_chunk_and_concurrent() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let path = "test_file";
        op.write(path, Buffer::from("HelloWorld")).await?;

        let reader = op.reader_with(path).chunk(2).concurrent(2).await.unwrap();
        let buf = reader.read(BytesRange::suffix(5)).await?;

        assert_eq!(buf.to_bytes(), b"World".as_slice());
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_into_futures_async_read_suffix() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let path = "test_file";
        op.write(path, Buffer::from("HelloWorld")).await?;

        let mut reader = op
            .reader(path)
            .await?
            .into_futures_async_read(BytesRange::suffix(5))
            .await?;
        let mut buf = Vec::new();
        futures::AsyncReadExt::read_to_end(&mut reader, &mut buf)
            .await
            .unwrap();

        assert_eq!(buf, b"World".as_slice());
        Ok(())
    }

    #[tokio::test]
    async fn test_reader_read_into() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
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
        Ok(())
    }

    #[tokio::test]
    async fn test_merge_ranges() -> Result<()> {
        let op = Operator::new(services::Memory::default()).unwrap();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader_with(path).gap(1).await.unwrap();

        let ranges = vec![0..10, 10..20, 21..30, 40..50, 40..60, 45..59];
        let merged = reader.merge_ranges(ranges);
        assert_eq!(merged, vec![0..30, 40..60]);

        let reader = op.reader_with(path).gap(0).await.unwrap();
        let ranges = vec![0..10, 10..20, 21..30, 40..50, 40..60, 45..59];
        let merged = reader.merge_ranges(ranges);
        assert_eq!(merged, vec![0..20, 21..30, 40..60]);

        assert_eq!(OpReader::new().with_gap(0).gap(), Some(0));
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch() -> Result<()> {
        let op = Operator::new(services::Memory::default()).unwrap();
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
        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_empty_ranges() -> Result<()> {
        let op = Operator::new(services::Memory::default()).unwrap();
        let path = "test_file";

        let content = gen_fixed_bytes(1024);
        op.write(path, content.clone())
            .await
            .expect("write must succeed");

        let reader = op.reader(path).await.unwrap();
        let result = reader
            .fetch(vec![])
            .await
            .expect("fetch with empty ranges must not panic");
        assert!(result.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_skips_zero_length_ranges() -> Result<()> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let reader = new_mock_reader(
            Bytes::from_static(b"abcdef"),
            OpReader::new(),
            calls.clone(),
        );

        let bufs = reader.fetch(vec![0..0, 1..3, 5..5]).await?;

        assert!(bufs[0].is_empty());
        assert_eq!(bufs[1].to_bytes(), b"bc".as_slice());
        assert!(bufs[2].is_empty());
        assert_eq!(*calls.lock().unwrap(), vec![BytesRange::new(1, Some(2))]);

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_plans_merged_ranges_with_chunk() -> Result<()> {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let reader = new_mock_reader(
            Bytes::from_static(b"abcdef"),
            OpReader::new()
                .with_gap(1)
                .with_chunk(4)
                .with_concurrent(2)
                .with_prefetch(1),
            calls.clone(),
        );

        let bufs = reader.fetch(vec![0..2, 3..6]).await?;

        assert_eq!(bufs[0].to_bytes(), b"ab".as_slice());
        assert_eq!(bufs[1].to_bytes(), b"def".as_slice());

        let mut calls = calls.lock().unwrap().clone();
        calls.sort_by_key(|range| range.offset());
        assert_eq!(
            calls,
            vec![BytesRange::new(0, Some(4)), BytesRange::new(4, Some(2))]
        );

        Ok(())
    }
}
