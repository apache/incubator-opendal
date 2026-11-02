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

use std::sync::Arc;

use bytes::Buf;
use futures::TryStreamExt;

use crate::raw::oio::Write;
use crate::raw::*;
use crate::*;

/// WriteContext holds the immutable context for give write operation.
pub struct WriteContext {
    /// The composed context used to execute operations.
    ctx: OperationContext,
    /// The composed service to the underlying object storage.
    srv: Servicer,
    /// Path to the file.
    path: String,
    /// Arguments for the write operation.
    args: OpWrite,
    /// Options for the writer.
    options: OpWriter,
}

impl WriteContext {
    /// Create a new WriteContext.
    #[inline]
    pub fn new(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        args: OpWrite,
        options: OpWriter,
    ) -> Self {
        Self {
            ctx,
            srv,
            path,
            args,
            options,
        }
    }

    /// Get the path.
    #[inline]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Get the composed operation context.
    #[inline]
    pub(crate) fn context(&self) -> &OperationContext {
        &self.ctx
    }

    /// Get the composed service.
    #[inline]
    pub(crate) fn service(&self) -> &Servicer {
        &self.srv
    }

    /// Get the arguments.
    #[inline]
    pub fn args(&self) -> &OpWrite {
        &self.args
    }

    /// Get the options.
    #[inline]
    pub fn options(&self) -> &OpWriter {
        &self.options
    }

    /// Calculate the chunk size for this write process.
    ///
    /// Returns the chunk size and if the chunk size is exact.
    fn calculate_chunk_size(&self) -> (Option<usize>, bool) {
        let cap = self.srv.capability();

        let exact = self.options().chunk().is_some();
        let chunk_size = self
            .options()
            .chunk()
            .or(cap.write_multi_min_size)
            .map(|mut size| {
                if let Some(v) = cap.write_multi_max_size {
                    size = size.min(v);
                }
                if let Some(v) = cap.write_multi_min_size {
                    size = size.max(v);
                }

                size
            });

        (chunk_size, exact)
    }
}

pub struct WriteGenerator<W> {
    w: W,

    ctx: Option<Arc<WriteContext>>,

    /// The size for buffer, we will flush the underlying storage at the size of this buffer.
    chunk_size: Option<usize>,
    /// If `exact` is true, the size of the data written to the underlying storage is
    /// exactly `chunk_size` bytes.
    exact: bool,
    /// Whether the composed writer can copy source ranges directly.
    can_copy_from: bool,
    buffer: oio::QueueBuf,
}

impl WriteGenerator<oio::Writer> {
    /// Create a new exact buf writer.
    pub fn create(ctx: Arc<WriteContext>) -> Result<Self> {
        let (chunk_size, exact) = ctx.calculate_chunk_size();
        let can_copy_from = ctx.service().capability().write_can_copy_from && !ctx.args().append();
        let w = ctx.srv.write(&ctx.ctx, ctx.path(), ctx.args().clone())?;

        Ok(Self {
            w,
            ctx: Some(ctx),
            chunk_size,
            exact,
            can_copy_from,
            buffer: oio::QueueBuf::new(),
        })
    }

    /// Allow building from existing oio::Writer for easier testing.
    #[cfg(test)]
    fn new(w: oio::Writer, chunk_size: Option<usize>, exact: bool) -> Self {
        Self {
            w,
            ctx: None,
            chunk_size,
            exact,
            can_copy_from: false,
            buffer: oio::QueueBuf::new(),
        }
    }
}

impl WriteGenerator<oio::Writer> {
    /// Write the entire buffer into writer.
    pub async fn write(&mut self, mut bs: Buffer) -> Result<usize> {
        let Some(chunk_size) = self.chunk_size else {
            let size = bs.len();
            self.w.write(bs).await?;
            return Ok(size);
        };

        if self.buffer.len() + bs.len() < chunk_size {
            let size = bs.len();
            self.buffer.push(bs);
            return Ok(size);
        }

        // Condition:
        // - exact is false
        // - buffer + bs is larger than chunk_size.
        // Action:
        // - write buffer + bs directly.
        if !self.exact {
            let fill_size = bs.len();
            self.buffer.push(bs);
            let buf = self.buffer.take().collect();
            self.w.write(buf).await?;
            return Ok(fill_size);
        }

        // Condition:
        // - exact is true: we need write buffer in exact chunk size.
        // - buffer is larger than chunk_size
        //   - in exact mode, the size must be chunk_size, use `>=` just for safe coding.
        // Action:
        // - write existing buffer in chunk_size to make more rooms for writing data.
        if self.buffer.len() >= chunk_size {
            let buf = self.buffer.take().collect();
            self.w.write(buf).await?;
        }

        // Condition
        // - exact is true.
        // - buffer size must lower than chunk_size.
        // Action:
        // - write bs to buffer with remaining size.
        let remaining = chunk_size - self.buffer.len();
        bs.truncate(remaining);
        let n = bs.len();
        self.buffer.push(bs);
        Ok(n)
    }

    fn reader(&self, path: &str, args: OpRead, options: OpReader) -> Result<Reader> {
        let ctx = self
            .ctx
            .as_ref()
            .expect("write context must exist outside tests");
        let reader = ctx.service().read(ctx.context(), path, args.clone())?;
        Ok(Reader::new(ReadContext::new(
            ctx.context().clone(),
            ctx.service().clone(),
            path.to_string(),
            args,
            options,
            reader,
        )))
    }

    async fn stream_range(
        &mut self,
        path: &str,
        args: OpRead,
        options: OpReader,
        range: BytesRange,
    ) -> Result<()> {
        let reader = self.reader(path, args, options)?;
        let mut stream = reader.into_stream(range).await?;
        while let Some(mut bs) = stream.try_next().await? {
            while !bs.is_empty() {
                let n = self.write(bs.clone()).await?;
                bs.advance(n);
            }
        }
        Ok(())
    }

    /// Copy a source range into this writer, using native execution when possible.
    pub async fn copy_from(
        &mut self,
        path: &str,
        args: OpRead,
        options: OpReader,
        range: BytesRange,
    ) -> Result<()> {
        const MAX_COPY_FROM_SIZE: u64 = 5 * 1024 * 1024 * 1024;

        if let Some(size) = range.size()
            && size > MAX_COPY_FROM_SIZE
        {
            return Err(Error::new(
                ErrorKind::RangeNotSatisfied,
                "copy_from range exceeds the 5 GiB per-call limit",
            )
            .with_context("size", size));
        }

        let reader = self.reader(path, args.clone(), options.clone())?;
        let absolute = reader.parse_into_range(range).await?;
        let size = absolute.end - absolute.start;
        if size > MAX_COPY_FROM_SIZE {
            return Err(Error::new(
                ErrorKind::RangeNotSatisfied,
                "copy_from range exceeds the 5 GiB per-call limit",
            )
            .with_context("size", size));
        }
        if size == 0 {
            return Ok(());
        }

        let Some(chunk_size) = self.chunk_size.filter(|_| self.can_copy_from) else {
            return self
                .stream_range(
                    path,
                    args,
                    options,
                    BytesRange::new(absolute.start, Some(size)),
                )
                .await;
        };

        let mut offset = absolute.start;
        let mut remaining = size;

        if self.buffer.len() >= chunk_size {
            let buf = self.buffer.take().collect();
            self.w.write(buf).await?;
        }

        if !self.buffer.is_empty() {
            let boundary_size = (chunk_size as u64 - self.buffer.len() as u64).min(remaining);
            self.stream_range(
                path,
                args.clone(),
                options.clone(),
                BytesRange::new(offset, Some(boundary_size)),
            )
            .await?;
            offset += boundary_size;
            remaining -= boundary_size;
        }

        while remaining >= chunk_size as u64 {
            let part_size = if self.exact {
                chunk_size as u64
            } else {
                remaining
            };
            let part_range = BytesRange::new(offset, Some(part_size));
            self.w.copy_from(path, args.clone(), part_range).await?;
            offset += part_size;
            remaining -= part_size;
        }

        if remaining > 0 {
            self.stream_range(
                path,
                args,
                options,
                BytesRange::new(offset, Some(remaining)),
            )
            .await?;
        }
        Ok(())
    }

    /// Finish the write process.
    pub async fn close(&mut self) -> Result<Metadata> {
        loop {
            if self.buffer.is_empty() {
                break;
            }

            let buf = self.buffer.take().collect();
            self.w.write(buf).await?;
        }

        self.w.close().await
    }

    /// Abort the write process.
    pub async fn abort(&mut self) -> Result<()> {
        self.buffer.clear();
        self.w.abort().await
    }
}

#[cfg(test)]
mod tests {
    use asyncband::mutex::Mutex;
    use bytes::Buf;
    use bytes::BufMut;
    use bytes::Bytes;
    use log::debug;
    use logforth::append::Testing;
    use logforth::filter::rustlog::RustLogFilterBuilder;
    use logforth::layout::TextLayout;
    use pretty_assertions::assert_eq;
    use rand::{Rng, RngExt, rng};
    use sha2::Digest;
    use sha2::Sha256;

    use super::*;
    use crate::raw::oio::Write;

    fn sha256_digest(data: impl AsRef<[u8]>) -> String {
        use std::fmt::Write;

        let digest = Sha256::digest(data);
        let mut output = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut output, "{byte:02x}").expect("writing to String must succeed");
        }
        output
    }

    struct MockWriter {
        buf: Arc<Mutex<Vec<u8>>>,
        write_sizes: Arc<Mutex<Vec<usize>>>,
    }

    impl Write for MockWriter {
        async fn write(&mut self, bs: Buffer) -> Result<()> {
            let size = bs.len();
            debug!("test_fuzz_exact_buf_writer: flush size: {}", size);

            let mut write_sizes = self.write_sizes.lock().await;
            write_sizes.push(size);

            let mut buf = self.buf.lock().await;
            buf.put(bs);
            Ok(())
        }

        async fn close(&mut self) -> Result<Metadata> {
            Ok(MetadataBuilder::unknown().build())
        }

        async fn abort(&mut self) -> Result<()> {
            Ok(())
        }
    }

    fn setup() {
        let _ = logforth::starter_log::builder()
            .dispatch(|d| {
                d.filter(RustLogFilterBuilder::from_default_env().build())
                    .append(Testing::default().with_layout(TextLayout::default()))
            })
            .try_apply();
    }

    #[tokio::test]
    async fn test_exact_buf_writer_short_write() -> Result<()> {
        setup();

        let mut rng = rng();
        let mut expected = vec![0; 5];
        rng.fill_bytes(&mut expected);

        let buf = Arc::new(Mutex::new(vec![]));
        let mut w = WriteGenerator::new(
            Box::new(MockWriter {
                buf: buf.clone(),
                write_sizes: Arc::new(Mutex::new(vec![])),
            }),
            Some(10),
            true,
        );

        let mut bs = Bytes::from(expected.clone());
        while !bs.is_empty() {
            let n = w.write(bs.clone().into()).await?;
            bs.advance(n);
        }

        w.close().await?;

        let buf = buf.lock().await;
        assert_eq!(buf.len(), expected.len());
        assert_eq!(sha256_digest(&*buf), sha256_digest(&expected));
        Ok(())
    }

    #[tokio::test]
    async fn test_inexact_buf_writer_large_write() -> Result<()> {
        setup();

        let buf = Arc::new(Mutex::new(vec![]));
        let mut w = WriteGenerator::new(
            Box::new(MockWriter {
                buf: buf.clone(),
                write_sizes: Arc::new(Mutex::new(vec![])),
            }),
            Some(10),
            false,
        );

        let mut rng = rng();
        let mut expected = vec![0; 15];
        rng.fill_bytes(&mut expected);

        let bs = Bytes::from(expected.clone());
        // The MockWriter always returns the first chunk size.
        let n = w.write(bs.into()).await?;
        assert_eq!(expected.len(), n);

        w.close().await?;

        let buf = buf.lock().await;
        assert_eq!(buf.len(), expected.len());
        assert_eq!(sha256_digest(&*buf), sha256_digest(&expected));
        Ok(())
    }

    #[tokio::test]
    async fn test_inexact_buf_writer_combine_small() -> Result<()> {
        setup();

        let buf = Arc::new(Mutex::new(vec![]));
        let mut w = WriteGenerator::new(
            Box::new(MockWriter {
                buf: buf.clone(),
                write_sizes: Arc::new(Mutex::new(vec![])),
            }),
            Some(10),
            false,
        );

        let mut rng = rng();
        let mut expected = vec![];

        let mut new_content = |size| {
            let mut content = vec![0; size];
            rng.fill_bytes(&mut content);
            expected.extend_from_slice(&content);
            Bytes::from(content)
        };

        // content > chunk size.
        let content = new_content(15);
        assert_eq!(15, w.write(content.into()).await?);
        // content < chunk size.
        let content = new_content(5);
        assert_eq!(5, w.write(content.into()).await?);
        // content > chunk size, but 5 bytes in queue.
        let content = new_content(15);
        // The MockWriter can send all 15 bytes together, so we can only advance 5 bytes.
        assert_eq!(15, w.write(content.clone().into()).await?);

        w.close().await?;

        let buf = buf.lock().await;
        assert_eq!(buf.len(), expected.len());
        assert_eq!(sha256_digest(&*buf), sha256_digest(&expected));
        Ok(())
    }

    #[tokio::test]
    async fn test_inexact_buf_writer_queue_remaining() -> Result<()> {
        setup();

        let buf = Arc::new(Mutex::new(vec![]));
        let mut w = WriteGenerator::new(
            Box::new(MockWriter {
                buf: buf.clone(),
                write_sizes: Arc::new(Mutex::new(vec![])),
            }),
            Some(10),
            false,
        );

        let mut rng = rng();
        let mut expected = vec![];

        let mut new_content = |size| {
            let mut content = vec![0; size];
            rng.fill_bytes(&mut content);
            expected.extend_from_slice(&content);
            Bytes::from(content)
        };

        // content > chunk size.
        let content = new_content(15);
        assert_eq!(15, w.write(content.into()).await?);
        // content < chunk size.
        let content = new_content(5);
        assert_eq!(5, w.write(content.into()).await?);
        // content < chunk size.
        let content = new_content(3);
        assert_eq!(3, w.write(content.into()).await?);
        // content > chunk size, but can send all chunks in the queue.
        let content = new_content(15);
        assert_eq!(15, w.write(content.clone().into()).await?);

        w.close().await?;

        let buf = buf.lock().await;
        assert_eq!(buf.len(), expected.len());
        assert_eq!(sha256_digest(&*buf), sha256_digest(&expected));
        Ok(())
    }

    #[tokio::test]
    async fn test_fuzz_exact_buf_writer() -> Result<()> {
        setup();

        let mut rng = rng();
        let mut expected = vec![];

        let buf = Arc::new(Mutex::new(vec![]));
        let buffer_size = rng.random_range(1..10);
        let mut writer = WriteGenerator::new(
            Box::new(MockWriter {
                buf: buf.clone(),
                write_sizes: Arc::new(Mutex::new(vec![])),
            }),
            Some(buffer_size),
            true,
        );
        debug!("test_fuzz_exact_buf_writer: buffer size: {buffer_size}");

        for _ in 0..1000 {
            let size = rng.random_range(1..20);
            debug!("test_fuzz_exact_buf_writer: write size: {size}");
            let mut content = vec![0; size];
            rng.fill_bytes(&mut content);

            expected.extend_from_slice(&content);

            let mut bs = Bytes::from(content.clone());
            while !bs.is_empty() {
                let n = writer.write(bs.clone().into()).await?;
                bs.advance(n);
            }
        }
        writer.close().await?;

        let buf = buf.lock().await;
        assert_eq!(buf.len(), expected.len());
        assert_eq!(sha256_digest(&*buf), sha256_digest(&expected));
        Ok(())
    }

    /// Test that when writing a large buffer in exact mode, it gets split into chunks.
    ///
    /// This test verifies that WriteGenerator correctly handles large buffers by
    /// splitting them into chunks of the configured chunk size, rather than writing
    /// everything at once.
    #[tokio::test]
    async fn test_exact_buf_writer_large_buffer_splits_into_chunks() -> Result<()> {
        setup();

        let chunk_size = 10;
        let large_buffer_size = 25; // 2.5x chunk_size

        let buf = Arc::new(Mutex::new(vec![]));
        let write_sizes = Arc::new(Mutex::new(vec![]));
        let mut writer = WriteGenerator::new(
            Box::new(MockWriter {
                buf: buf.clone(),
                write_sizes: write_sizes.clone(),
            }),
            Some(chunk_size),
            true, // exact mode
        );

        let mut rng = rng();
        let mut expected = vec![0; large_buffer_size];
        rng.fill_bytes(&mut expected);

        let bs = Bytes::from(expected.clone());
        // In exact mode, a large buffer should be written in chunks.
        // We need to call write multiple times until all data is written.
        let mut remaining = bs.clone();
        while !remaining.is_empty() {
            let n = writer.write(remaining.clone().into()).await?;
            remaining.advance(n);
        }

        writer.close().await?;

        // Verify all data was written
        let buf = buf.lock().await;
        assert_eq!(buf.len(), expected.len());
        assert_eq!(sha256_digest(&*buf), sha256_digest(&expected));

        // Verify that writes were split into chunks (except possibly the last one)
        let write_sizes = write_sizes.lock().await;
        // In exact mode, all writes except the last should be exactly chunk_size
        // The last write might be smaller if there's a remainder
        for (i, &size) in write_sizes.iter().enumerate() {
            if i < write_sizes.len() - 1 {
                // All writes except the last should be exactly chunk_size
                assert_eq!(
                    size, chunk_size,
                    "Write {} should be exactly chunk_size {}, but was {}",
                    i, chunk_size, size
                );
            } else {
                // Last write should be <= chunk_size
                assert!(
                    size <= chunk_size,
                    "Last write should be <= chunk_size {}, but was {}",
                    chunk_size,
                    size
                );
            }
        }

        // Verify we got the expected number of writes
        let expected_writes = large_buffer_size.div_ceil(chunk_size);
        assert_eq!(
            write_sizes.len(),
            expected_writes,
            "Expected {} writes, but got {}",
            expected_writes,
            write_sizes.len()
        );

        Ok(())
    }
}
