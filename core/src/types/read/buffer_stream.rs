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
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures::stream::Buffered;
use futures::stream::FusedStream;
use futures::stream::Iter;
use futures::stream::{self};
use futures::Stream;
use futures::StreamExt;

use crate::raw::*;
use crate::*;

/// FutureIterator is an iterator that returns future generated by `Reader`'s
/// read.
struct FutureIterator {
    r: oio::Reader,
    chunk: Option<usize>,

    offset: u64,
    end: u64,
}

impl FutureIterator {
    #[inline]
    fn new(r: oio::Reader, chunk: Option<usize>, range: Range<u64>) -> Self {
        FutureIterator {
            r,
            chunk,
            offset: range.start,
            end: range.end,
        }
    }
}

impl Iterator for FutureIterator {
    type Item = BoxedFuture<'static, Result<Buffer>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.end {
            return None;
        }

        let offset = self.offset;
        let mut limit = (self.end - self.offset) as usize;
        if let Some(chunk) = self.chunk {
            limit = limit.min(chunk)
        }

        // Update self.offset before building future.
        self.offset += limit as u64;
        let r = self.r.clone();
        let fut = async move {
            let buf = r.read_at_dyn(offset, limit).await?;
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
pub struct BufferStream(Buffered<Iter<FutureIterator>>);

impl BufferStream {
    /// Create a new buffer stream from given reader.
    #[inline]
    pub(crate) fn new(r: oio::Reader, options: OpReader, range: Range<u64>) -> Self {
        let iter = FutureIterator::new(r, options.chunk(), range);
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

impl FusedStream for BufferStream {
    fn is_terminated(&self) -> bool {
        self.0.is_terminated()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Buf;
    use bytes::Bytes;
    use futures::TryStreamExt;
    use pretty_assertions::assert_eq;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn test_trait() {
        let v = BufferStream::new(Arc::new(Buffer::new()), OpReader::new(), 4..8);

        let _: Box<dyn Unpin + MaybeSend + 'static> = Box::new(v);
    }

    #[test]
    fn test_future_iterator() {
        let r: oio::Reader = Arc::new(Buffer::new());

        let it = FutureIterator::new(r.clone(), Some(1), 1..3);
        let futs: Vec<_> = it.collect();
        assert_eq!(futs.len(), 2);
    }

    #[tokio::test]
    async fn test_buffer_stream() {
        let r: oio::Reader = Arc::new(Buffer::from(vec![
            Bytes::from("Hello"),
            Bytes::from("World"),
        ]));

        let s = BufferStream::new(r, OpReader::new(), 4..8);
        let bufs: Vec<_> = s.try_collect().await.unwrap();
        assert_eq!(bufs.len(), 1);
        assert_eq!(bufs[0].chunk(), "o".as_bytes());

        let buf: Buffer = bufs.into_iter().flatten().collect();
        assert_eq!(buf.len(), 4);
        assert_eq!(&buf.to_vec(), "oWor".as_bytes());
    }
}
