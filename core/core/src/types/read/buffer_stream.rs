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

use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use futures::Stream;
use futures::ready;

use crate::raw::oio::Read as _;
use crate::raw::oio::ReadStream as _;
use crate::raw::*;
use crate::*;

/// StreamingReader will stream the content of the file without reading into
/// memory first.
///
/// StreamingReader is good for small memory footprint and optimized for latency.
pub struct StreamingReader {
    generator: ReadGenerator,
    reader: Option<Box<dyn oio::ReadStreamDyn>>,
}

impl StreamingReader {
    /// Create a new streaming reader.
    #[inline]
    fn new(ctx: Arc<ReadContext>, range: BytesRange) -> Self {
        let generator = ReadGenerator::new(ctx, range);
        Self {
            generator,
            reader: None,
        }
    }

    async fn prepare_metadata(&mut self) -> Result<()> {
        if self.generator.metadata().is_some() {
            return Ok(());
        }

        if self.reader.is_none() {
            self.reader = self.generator.next_reader().await?;
        }

        Ok(())
    }

    async fn metadata(&mut self) -> Result<Metadata> {
        self.prepare_metadata().await?;

        self.generator
            .metadata()
            .cloned()
            .ok_or_else(|| Error::new(ErrorKind::Unsupported, "read metadata is not available"))
    }
}

impl oio::ReadStream for StreamingReader {
    async fn read(&mut self) -> Result<Buffer> {
        loop {
            if self.reader.is_none() {
                self.reader = self.generator.next_reader().await?;
            }

            let Some(r) = self.reader.as_mut() else {
                return Ok(Buffer::new());
            };

            let buf = r.read_dyn().await?;
            // Reset reader to None if this reader returns empty buffer.
            if buf.is_empty() {
                self.reader = None;
                continue;
            } else {
                return Ok(buf);
            }
        }
    }
}

struct ChunkedReadInput {
    ctx: Arc<ReadContext>,
    range: BytesRange,
    reader: Option<Box<dyn oio::ReadStreamDyn>>,
}

/// ChunkedReader will read the file in chunks.
///
/// ChunkedReader is good for concurrent read and optimized for throughput.
pub struct ChunkedReader {
    ctx: Arc<ReadContext>,
    offset: u64,
    remaining: Option<u64>,
    opened: Option<ChunkedReadInput>,
    tasks: ConcurrentTasks<ChunkedReadInput, Buffer>,
    done: bool,
}

impl ChunkedReader {
    /// Create a new chunked reader.
    ///
    /// # Notes
    ///
    /// We don't need to handle `Executor::timeout` since we are outside the layer.
    fn new(ctx: Arc<ReadContext>, range: BytesRange) -> Self {
        let tasks = ConcurrentTasks::new(
            ctx.context().executor().clone(),
            ctx.options().concurrent(),
            ctx.options().prefetch(),
            |mut input: ChunkedReadInput| {
                Box::pin(async move {
                    let result = if let Some(mut reader) = input.reader.take() {
                        reader.read_all().await
                    } else {
                        match input.ctx.reader().read(input.range).await {
                            Ok((rp, buffer)) => {
                                input.ctx.observe_read_response(rp);
                                Ok(buffer)
                            }
                            Err(err) => Err(err),
                        }
                    };
                    (input, result)
                })
            },
        );
        Self {
            ctx,
            offset: range.offset(),
            remaining: range.size(),
            opened: None,
            tasks,
            done: false,
        }
    }

    async fn prepare_metadata(&mut self) -> Result<()> {
        if self.ctx.metadata().is_some() {
            return Ok(());
        }

        if self.opened.is_none() {
            if let Some(range) = self.next_range() {
                let (rp, reader) = self.ctx.reader().open(range).await?;
                self.ctx.observe_read_response(rp);
                self.opened = Some(ChunkedReadInput {
                    ctx: self.ctx.clone(),
                    range,
                    reader: Some(reader),
                });
            } else {
                self.done = true;
            }
        }

        Ok(())
    }

    async fn metadata(&mut self) -> Result<Metadata> {
        self.prepare_metadata().await?;

        self.ctx
            .metadata()
            .cloned()
            .ok_or_else(|| Error::new(ErrorKind::Unsupported, "read metadata is not available"))
    }

    /// Generate the next range to read, advancing internal state.
    fn next_range(&mut self) -> Option<BytesRange> {
        if self.remaining == Some(0) {
            return None;
        }

        let next_offset = self.offset;
        let next_size = match self.remaining {
            None => {
                self.remaining = Some(0);
                None
            }
            Some(remaining) => {
                let read_size = self
                    .ctx
                    .options()
                    .chunk()
                    .map_or(remaining, |chunk| remaining.min(chunk as u64));
                self.offset += read_size;
                self.remaining = Some(remaining - read_size);
                Some(read_size)
            }
        };

        Some(BytesRange::new(next_offset, next_size))
    }
}

impl oio::ReadStream for ChunkedReader {
    async fn read(&mut self) -> Result<Buffer> {
        while self.tasks.has_remaining() && !self.done {
            if let Some(input) = self.opened.take() {
                self.tasks.execute(input).await?;
            } else if let Some(range) = self.next_range() {
                self.tasks
                    .execute(ChunkedReadInput {
                        ctx: self.ctx.clone(),
                        range,
                        reader: None,
                    })
                    .await?;
            } else {
                self.done = true;
                break;
            }
            if self.tasks.has_result() {
                break;
            }
        }

        let Some(buffer) = self.tasks.next().await.transpose()? else {
            return Ok(Buffer::new());
        };

        Ok(buffer)
    }
}

/// BufferStream is a stream of buffers, created by [`Reader::into_stream`]
///
/// `BufferStream` implements `Stream` trait.
pub struct BufferStream {
    ctx: Arc<ReadContext>,
    /// # Notes to maintainers
    ///
    /// The underlying reader is either a StreamingReader or a ChunkedReader.
    ///
    /// - If chunk is None, BufferStream will use StreamingReader to iterate
    ///   data in streaming way.
    /// - Otherwise, BufferStream will use ChunkedReader to read data in chunks.
    state: State,
}

#[allow(clippy::large_enum_variant)]
enum State {
    Idle(Option<TwoWays<StreamingReader, ChunkedReader>>),
    Reading(BoxedStaticFuture<(TwoWays<StreamingReader, ChunkedReader>, Result<Buffer>)>),
}

impl BufferStream {
    /// Create a new buffer stream with already calculated offset and size.
    pub(crate) fn new(ctx: Arc<ReadContext>, offset: u64, size: Option<u64>) -> Self {
        debug_assert!(
            size.is_some() || ctx.options().chunk().is_none(),
            "size must be known if chunk is set"
        );

        let reader = if ctx.options().chunk().is_some() {
            TwoWays::Two(ChunkedReader::new(
                ctx.clone(),
                BytesRange::new(offset, size),
            ))
        } else {
            TwoWays::One(StreamingReader::new(
                ctx.clone(),
                BytesRange::new(offset, size),
            ))
        };

        Self {
            ctx,
            state: State::Idle(Some(reader)),
        }
    }

    /// Create a new buffer stream with given range bound.
    ///
    /// If users is going to perform chunked read but the read size is unknown, we will parse into
    /// range first.
    pub(crate) async fn create(
        ctx: Arc<ReadContext>,
        range: impl Into<BytesRange>,
    ) -> Result<Self> {
        let range = range.into();
        let reader = if ctx.options().chunk().is_some() {
            let range = ctx.parse_into_range(range).await?;
            TwoWays::Two(ChunkedReader::new(ctx.clone(), range.into()))
        } else {
            TwoWays::One(StreamingReader::new(ctx.clone(), range))
        };

        Ok(Self {
            ctx,
            state: State::Idle(Some(reader)),
        })
    }

    /// Get metadata for this stream.
    ///
    /// Calling this method opens the underlying read request if needed.
    /// Returns [`ErrorKind::Unsupported`] if the underlying service doesn't
    /// return metadata while opening the read operation.
    pub async fn metadata(&mut self) -> Result<Metadata> {
        if let Some(metadata) = self.ctx.metadata() {
            return Ok(metadata.clone());
        }

        match std::mem::replace(&mut self.state, State::Idle(None)) {
            State::Idle(reader) => {
                let mut reader = reader.expect("reader must exist while idle");
                let prepared = match &mut reader {
                    TwoWays::One(v) => v.metadata().await,
                    TwoWays::Two(v) => v.metadata().await,
                };
                self.state = State::Idle(Some(reader));
                prepared
            }
            State::Reading(fut) => {
                self.state = State::Reading(fut);
                self.ctx.metadata().cloned().ok_or_else(|| {
                    Error::new(ErrorKind::Unsupported, "read metadata is not available")
                })
            }
        }
    }
}

impl Stream for BufferStream {
    type Item = Result<Buffer>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            match &mut this.state {
                State::Idle(reader) => {
                    let mut reader = reader.take().unwrap();
                    let fut = async move {
                        let ret = reader.read().await;
                        (reader, ret)
                    };
                    this.state = State::Reading(Box::pin(fut));
                }
                State::Reading(fut) => {
                    let fut = fut.as_mut();
                    let (reader, buf) = ready!(fut.poll(cx));
                    this.state = State::Idle(Some(reader));
                    return match buf {
                        Ok(buf) if buf.is_empty() => Poll::Ready(None),
                        Ok(buf) => Poll::Ready(Some(Ok(buf))),
                        Err(err) => Poll::Ready(Some(Err(err))),
                    };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Buf;
    use bytes::Bytes;
    use futures::TryStreamExt;
    use pretty_assertions::assert_eq;

    use super::*;

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

    #[tokio::test]
    async fn test_trait() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        let ctx = op.context().clone();
        let srv = op.service().clone();
        let ctx = Arc::new(new_read_context(ctx, srv, "test", OpReader::new())?);
        let v = BufferStream::create(ctx, 4..8).await?;

        let _: Box<dyn Unpin + MaybeSend + 'static> = Box::new(v);

        Ok(())
    }

    #[tokio::test]
    async fn test_buffer_stream() -> Result<()> {
        let op = Operator::via_iter(services::MEMORY_SCHEME, [])?;
        op.write(
            "test",
            Buffer::from(vec![Bytes::from("Hello"), Bytes::from("World")]),
        )
        .await?;

        let ctx = op.context().clone();
        let srv = op.service().clone();
        let ctx = Arc::new(new_read_context(ctx, srv, "test", OpReader::new())?);

        let s = BufferStream::create(ctx, 4..8).await?;
        let bufs: Vec<_> = s.try_collect().await.unwrap();
        assert_eq!(bufs.len(), 1);
        assert_eq!(bufs[0].chunk(), "o".as_bytes());

        let buf: Buffer = bufs.into_iter().flatten().collect();
        assert_eq!(buf.len(), 4);
        assert_eq!(&buf.to_vec(), "oWor".as_bytes());

        Ok(())
    }
}
