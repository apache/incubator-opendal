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

use crate::raw::oio::{StreamExt, Streamer};
use crate::raw::*;
use crate::*;
use async_trait::async_trait;
use bytes::Bytes;

/// AtLeastBufWrite is used to implement [`Write`] based on at least buffer.
///
/// Users can wrap a writer and a buffer together.
pub struct AtLeastBufWriter<W: oio::Write> {
    inner: W,

    /// The total size of the data.
    ///
    /// If the total size is known, we will write to underlying storage directly without buffer it
    /// when possible.
    total_size: Option<u64>,

    /// The size for buffer, we will flush the underlying storage if the buffer is full.
    buffer_size: usize,
    buffer: oio::ChunkedCursor,
}

impl<W: oio::Write> AtLeastBufWriter<W> {
    /// Create a new at least buf writer.
    pub fn new(inner: W, buffer_size: usize) -> Self {
        Self {
            inner,
            total_size: None,
            buffer_size,
            buffer: oio::ChunkedCursor::new(),
        }
    }

    /// Configure the total size for writer.
    pub fn with_total_size(mut self, total_size: Option<u64>) -> Self {
        self.total_size = total_size;
        self
    }
}

#[async_trait]
impl<W: oio::Write> oio::Write for AtLeastBufWriter<W> {
    async fn write(&mut self, bs: Bytes) -> Result<()> {
        // If total size is known and equals to given bytes, we can write it directly.
        if let Some(total_size) = self.total_size {
            if total_size == bs.len() as u64 {
                return self.inner.write(bs).await;
            }
        }

        // Push the bytes into the buffer if the buffer is not full.
        if self.buffer.len() + bs.len() < self.buffer_size {
            self.buffer.push(bs);
            return Ok(());
        }

        let mut buf = self.buffer.clone();
        buf.push(bs);

        self.inner
            .sink(buf.len() as u64, Box::new(buf))
            .await
            // Clear buffer if the write is successful.
            .map(|_| self.buffer.clear())
    }

    async fn sink(&mut self, size: u64, s: Streamer) -> Result<()> {
        // If total size is known and equals to given stream, we can write it directly.
        if let Some(total_size) = self.total_size {
            if total_size == size {
                return self.inner.sink(size, s).await;
            }
        }

        // Push the bytes into the buffer if the buffer is not full.
        if self.buffer.len() as u64 + size < self.buffer_size as u64 {
            self.buffer.push(s.collect().await?);
            return Ok(());
        }

        let buf = self.buffer.clone();
        let buffer_size = buf.len() as u64;
        let stream = buf.chain(s);

        self.inner
            .sink(buffer_size + size, Box::new(stream))
            .await
            // Clear buffer if the write is successful.
            .map(|_| self.buffer.clear())
    }

    async fn abort(&mut self) -> Result<()> {
        self.buffer.clear();
        self.inner.abort().await
    }

    async fn close(&mut self) -> Result<()> {
        if !self.buffer.is_empty() {
            self.inner
                .sink(self.buffer.len() as u64, Box::new(self.buffer.clone()))
                .await?;
            self.buffer.clear();
        }

        self.inner.close().await
    }
}
