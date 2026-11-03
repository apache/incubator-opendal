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
/// ```
/// use opendal_core::Operator;
/// use opendal_core::Result;
///
/// async fn test(op: Operator) -> Result<()> {
///     let mut w = op.writer("path/to/file").await?;
///     w.write(vec![1; 1024]).await?;
///     w.write(vec![2; 1024]).await?;
///     w.close().await?;
///     Ok(())
/// }
/// ```
///
/// ### Write like `Sink`
///
/// ```
/// use anyhow::Result;
/// use futures::SinkExt;
/// use opendal_core::Operator;
///
/// async fn test(op: Operator) -> Result<()> {
///     let mut w = op.writer("path/to/file").await?.into_bytes_sink();
///     w.send(vec![1; 1024].into()).await?;
///     w.send(vec![2; 1024].into()).await?;
///     w.close().await?;
///     Ok(())
/// }
/// ```
///
/// ### Write like `AsyncWrite`
///
/// ```
/// use anyhow::Result;
/// use futures::AsyncWriteExt;
/// use opendal_core::Operator;
///
/// async fn test(op: Operator) -> Result<()> {
///     let mut w = op.writer("path/to/file").await?.into_futures_async_write();
///     w.write(&vec![1; 1024]).await?;
///     w.write(&vec![2; 1024]).await?;
///     w.close().await?;
///     Ok(())
/// }
/// ```
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
///   on non-appendable file could return [`ErrorKind::Conflict`] error.
/// - Services that doesn't support append will return [`ErrorKind::Unsupported`] error when
///   creating writer with `append` enabled.
pub struct Writer {
    /// Keep a reference to write context in writer.
    ctx: Arc<WriteContext>,
    inner: WriteGenerator<oio::Writer>,
}

impl Writer {
    /// Create a new writer from an `oio::Writer`.
    pub(crate) fn new(ctx: WriteContext) -> Result<Self> {
        let ctx = Arc::new(ctx);
        let inner = WriteGenerator::create(ctx.clone())?;

        Ok(Self { ctx, inner })
    }

    /// Write [`Buffer`] into writer.
    ///
    /// This operation will write all data in given buffer into writer.
    ///
    /// ## Examples
    ///
    /// ```
    /// use bytes::Bytes;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let mut w = op.writer("hello.txt").await?;
    ///     // Buffer can be created from continues bytes.
    ///     w.write("hello, world").await?;
    ///     // Buffer can also be created from non-continues bytes.
    ///     w.write(vec![Bytes::from("hello,"), Bytes::from("world!")])
    ///         .await?;
    ///
    ///     // Make sure file has been written completely.
    ///     w.close().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn write(&mut self, bs: impl Into<Buffer>) -> Result<()> {
        let mut bs = bs.into();
        while !bs.is_empty() {
            let n = self.inner.write(bs.clone()).await?;
            bs.advance(n);
        }

        Ok(())
    }

    /// Write [`bytes::Buf`] into inner writer.
    ///
    /// This operation will write all data in given buffer into writer.
    ///
    /// # Notes
    ///
    /// This function iterates over each chunk in the `Buf` and collects them
    /// into a `Buffer`. For `Bytes` chunks, `copy_to_bytes` is zero-copy
    /// (only a ref-count increment). For other chunk types (e.g., `&[u8]`),
    /// the data will be copied.
    ///
    /// This approach minimizes copies for chained buffers (`Chain<Bytes, Bytes>`)
    /// where each individual chunk can be extracted without copying.
    pub async fn write_from(&mut self, bs: impl Buf) -> Result<()> {
        let mut bs = bs;

        // Fast path: single contiguous chunk.
        // If chunk length equals remaining bytes, the entire buffer is contiguous.
        // This avoids Vec allocation and loop overhead for the common case.
        if bs.chunk().len() == bs.remaining() {
            let bytes = bs.copy_to_bytes(bs.remaining());
            return self.write(Buffer::from(bytes)).await;
        }

        // Slow path: multiple chunks (e.g., Chain<T, U>).
        // Iterate over each chunk and collect them into a Vec<Bytes>.
        // For Bytes chunks, copy_to_bytes is zero-copy.
        // For other types, only the current chunk is copied, not the entire buffer.
        let mut chunks = Vec::new();
        while bs.has_remaining() {
            let chunk_len = bs.chunk().len();
            chunks.push(bs.copy_to_bytes(chunk_len));
        }
        self.write(Buffer::from(chunks)).await
    }

    /// Copy a range of an existing object into this writer.
    ///
    /// The source path is resolved by the same [`Operator`] that created this
    /// writer. Calls to `write` and `copy_from` may be interleaved, and their
    /// call order defines the destination bytes. OpenDAL uses a native
    /// writer-side copy when the composed service supports it and otherwise
    /// streams the range through the client. Check
    /// [`Capability::write_can_copy_from`] to determine whether eligible
    /// non-append writes use the native path.
    ///
    /// Each call accepts at most 5 GiB. Applications must split larger source
    /// ranges into multiple ordered calls.
    pub async fn copy_from(&mut self, path: &str, range: impl Into<BytesRange>) -> Result<()> {
        self.copy_from_options(
            path,
            options::ReadOptions {
                range: range.into(),
                ..Default::default()
            },
        )
        .await
    }

    /// Copy an existing object range with read conditions and execution options.
    pub async fn copy_from_options(
        &mut self,
        path: &str,
        options: options::ReadOptions,
    ) -> Result<()> {
        let path = normalize_path(path);
        if !validate_path(&path, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "copy source path is a directory")
                    .with_operation("Writer::copy_from")
                    .with_context("service", self.ctx.service().info().scheme())
                    .with_context("path", &path),
            );
        }
        let (range, args, options) = options.into();
        self.inner.copy_from(&path, args, options, range).await
    }

    /// Abort the writer and clean up all written data.
    ///
    /// ## Notes
    ///
    /// Abort should only be called when the writer is not closed or
    /// aborted, otherwise an unexpected error could be returned.
    pub async fn abort(&mut self) -> Result<()> {
        self.inner.abort().await
    }

    /// Close the writer and make sure all data have been committed.
    ///
    /// ## Notes
    ///
    /// Close should only be called when the writer is not closed or
    /// aborted, otherwise an unexpected error could be returned.
    pub async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().await
    }

    /// Convert this writer into a [`BufferSink`] that implements
    /// [`Sink<Buffer>`][futures::Sink].
    ///
    /// # Notes
    ///
    /// BufferSink is a zero-cost abstraction. The underlying writer
    /// will reuse the Bytes and won't perform any copy operation over data.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```
    /// use std::io;
    ///
    /// use bytes::Bytes;
    /// use futures::SinkExt;
    /// use opendal_core::Buffer;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut s = op.writer("hello.txt").await?.into_sink();
    ///     let bs = "Hello, World!".as_bytes();
    ///     s.send(Buffer::from(bs)).await?;
    ///     s.close().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Concurrent Write
    ///
    /// ```
    /// use std::io;
    ///
    /// use bytes::Bytes;
    /// use futures::SinkExt;
    /// use opendal_core::Buffer;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut w = op
    ///         .writer_with("hello.txt")
    ///         .concurrent(8)
    ///         .chunk(256)
    ///         .await?
    ///         .into_sink();
    ///     let bs = "Hello, World!".as_bytes();
    ///     w.send(Buffer::from(bs)).await?;
    ///     w.close().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn into_sink(self) -> BufferSink {
        BufferSink::new(self.inner)
    }

    /// Convert writer into [`FuturesAsyncWriter`] which implements [`futures::AsyncWrite`],
    ///
    /// # Notes
    ///
    /// FuturesAsyncWriter is not a zero-cost abstraction. The underlying writer
    /// requires an owned [`Buffer`], which involves an extra copy operation.
    ///
    /// FuturesAsyncWriter is required to call `close()` to make sure all
    /// data have been written to the storage.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```
    /// use std::io;
    ///
    /// use futures::io::AsyncWriteExt;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut w = op.writer("hello.txt").await?.into_futures_async_write();
    ///     let bs = "Hello, World!".as_bytes();
    ///     w.write_all(bs).await?;
    ///     w.close().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Concurrent Write
    ///
    /// ```
    /// use std::io;
    ///
    /// use futures::io::AsyncWriteExt;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut w = op
    ///         .writer_with("hello.txt")
    ///         .concurrent(8)
    ///         .chunk(256)
    ///         .await?
    ///         .into_futures_async_write();
    ///     let bs = "Hello, World!".as_bytes();
    ///     w.write_all(bs).await?;
    ///     w.close().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn into_futures_async_write(self) -> FuturesAsyncWriter {
        FuturesAsyncWriter::new(self.inner)
    }

    /// Convert writer into [`FuturesBytesSink`] which implements [`futures::Sink<Bytes>`].
    ///
    /// # Notes
    ///
    /// FuturesBytesSink is a zero-cost abstraction. The underlying writer
    /// will reuse the Bytes and won't perform any copy operation.
    ///
    /// # Examples
    ///
    /// ## Basic Usage
    ///
    /// ```
    /// use std::io;
    ///
    /// use bytes::Bytes;
    /// use futures::SinkExt;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut w = op.writer("hello.txt").await?.into_bytes_sink();
    ///     let bs = "Hello, World!".as_bytes();
    ///     w.send(Bytes::from(bs)).await?;
    ///     w.close().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// ## Concurrent Write
    ///
    /// ```
    /// use std::io;
    ///
    /// use bytes::Bytes;
    /// use futures::SinkExt;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn test(op: Operator) -> io::Result<()> {
    ///     let mut w = op
    ///         .writer_with("hello.txt")
    ///         .concurrent(8)
    ///         .chunk(256)
    ///         .await?
    ///         .into_bytes_sink();
    ///     let bs = "Hello, World!".as_bytes();
    ///     w.send(Bytes::from(bs)).await?;
    ///     w.close().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn into_bytes_sink(self) -> FuturesBytesSink {
        FuturesBytesSink::new(self.inner)
    }
}

#[cfg(test)]
mod tests {
    use bytes::{Buf, Bytes};
    use rand::{Rng, RngExt};

    use crate::Operator;
    use crate::services;

    fn gen_random_bytes() -> Vec<u8> {
        let mut rng = rand::rng();
        // Generate size between 1B..16MB.
        let size = rng.random_range(1..16 * 1024 * 1024);
        let mut content = vec![0; size];
        rng.fill_bytes(&mut content);
        content
    }

    #[tokio::test]
    async fn test_writer_write() {
        let op = Operator::new(services::Memory::default()).unwrap();
        let path = "test_file";

        let content = gen_random_bytes();
        let mut writer = op.writer(path).await.unwrap();
        writer
            .write(content.clone())
            .await
            .expect("write must succeed");
        writer.close().await.expect("close must succeed");

        let buf = op.read(path).await.expect("read to end mut succeed");

        assert_eq!(buf.to_bytes(), content);
    }

    #[tokio::test]
    async fn test_writer_write_from() {
        let op = Operator::new(services::Memory::default()).unwrap();
        let path = "test_file";

        let content = gen_random_bytes();
        let mut writer = op.writer(path).await.unwrap();
        writer
            .write_from(Bytes::from(content.clone()))
            .await
            .expect("write must succeed");
        writer.close().await.expect("close must succeed");

        let buf = op.read(path).await.expect("read to end mut succeed");

        assert_eq!(buf.to_bytes(), content);
    }

    #[tokio::test]
    async fn test_writer_write_from_chain() {
        let op = Operator::new(services::Memory::default()).unwrap();
        let path = "test_file";

        let part1 = Bytes::from(gen_random_bytes());
        let part2 = Bytes::from(gen_random_bytes());

        let mut chain_same = part1.clone().chain(part2.clone());
        let chain = part1.chain(part2);

        let mut writer = op.writer(path).await.unwrap();
        writer.write_from(chain).await.expect("write must succeed");
        writer.close().await.expect("close must succeed");

        let buf = op.read(path).await.expect("read to end mut succeed");

        assert_eq!(
            buf.to_bytes(),
            chain_same.copy_to_bytes(chain_same.remaining())
        );
    }
}
