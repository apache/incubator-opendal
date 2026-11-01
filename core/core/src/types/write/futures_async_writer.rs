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
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use futures::AsyncWrite;
use futures::SinkExt;

use crate::raw::*;
use crate::*;

/// FuturesIoAsyncWriter is the adapter of [`AsyncWrite`] for [`Writer`].
///
/// Users can use this adapter in cases where they need to use [`AsyncWrite`] related trait.
///
/// FuturesIoAsyncWriter also implements [`Unpin`], [`Send`] and [`Sync`]
pub struct FuturesAsyncWriter {
    sink: BufferSink,
    buf: oio::FlexBuf,
}

impl FuturesAsyncWriter {
    /// NOTE: don't allow users to create directly.
    #[inline]
    pub(crate) fn new(w: WriteGenerator<oio::Writer>) -> Self {
        FuturesAsyncWriter {
            sink: BufferSink::new(w),
            buf: oio::FlexBuf::new(256 * 1024),
        }
    }
}

impl AsyncWrite for FuturesAsyncWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();

        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        loop {
            let n = this.buf.put(buf);
            if n > 0 {
                return Poll::Ready(Ok(n));
            }

            ready!(this.sink.poll_ready_unpin(cx)).map_err(format_std_io_error)?;

            let bs = this.buf.get().expect("frozen buffer must be valid");
            this.sink
                .start_send_unpin(Buffer::from(bs))
                .map_err(format_std_io_error)?;
            this.buf.clean();
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        loop {
            // Make sure buf has been frozen.
            this.buf.freeze();
            let Some(bs) = this.buf.get() else {
                return Poll::Ready(Ok(()));
            };

            ready!(this.sink.poll_ready_unpin(cx)).map_err(format_std_io_error)?;
            this.sink
                .start_send_unpin(Buffer::from(bs))
                .map_err(format_std_io_error)?;
            this.buf.clean();
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        loop {
            // Make sure buf has been frozen.
            this.buf.freeze();
            let Some(bs) = this.buf.get() else {
                return this.sink.poll_close_unpin(cx).map_err(format_std_io_error);
            };

            ready!(this.sink.poll_ready_unpin(cx)).map_err(format_std_io_error)?;
            this.sink
                .start_send_unpin(Buffer::from(bs))
                .map_err(format_std_io_error)?;
            this.buf.clean();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::AsyncWriteExt;

    use super::*;
    use crate::raw::MaybeSend;

    #[tokio::test]
    async fn test_trait() {
        let op = Operator::via_iter(services::MEMORY_SCHEME, []).unwrap();

        let ctx = op.context().clone();
        let srv = op.service().clone();
        let ctx = Arc::new(WriteContext::new(
            ctx,
            srv,
            "test".to_string(),
            OpWrite::new(),
            OpWriter::new().with_chunk(1),
        ));
        let write_gen = WriteGenerator::create(ctx).unwrap();

        let v = FuturesAsyncWriter::new(write_gen);

        let _: Box<dyn Unpin + MaybeSend + Sync + 'static> = Box::new(v);
    }

    #[tokio::test]
    async fn test_empty_write() {
        let op = Operator::via_iter(services::MEMORY_SCHEME, []).unwrap();

        let mut w = op
            .writer("test_empty_write")
            .await
            .unwrap()
            .into_futures_async_write();

        assert_eq!(w.write(b"").await.unwrap(), 0);
        w.write_all(b"hello").await.unwrap();
        assert_eq!(w.write(b"").await.unwrap(), 0);
        w.close().await.unwrap();

        assert_eq!(
            op.read("test_empty_write").await.unwrap().to_vec(),
            b"hello"
        );
    }
}
