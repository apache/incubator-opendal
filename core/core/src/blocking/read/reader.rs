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

use bytes::BufMut;

use super::BufferIterator;
use super::StdBytesIterator;
use super::StdReader;
use crate::Reader as AsyncReader;
use crate::*;

/// BlockingReader is designed to read data from given path in an blocking
/// manner.
#[derive(Clone)]
pub struct Reader {
    handle: tokio::runtime::Handle,
    inner: Option<AsyncReader>,
}

impl Reader {
    /// Create a new blocking reader.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) fn new(handle: tokio::runtime::Handle, inner: AsyncReader) -> Self {
        Reader {
            handle,
            inner: Some(inner),
        }
    }

    /// Read give range from reader into [`Buffer`].
    ///
    /// This operation is zero-copy, which means it keeps the [`bytes::Bytes`] returned by underlying
    /// storage services without any extra copy or intensive memory allocations.
    ///
    /// # Notes
    ///
    /// - Buffer length smaller than range means we have reached the end of file.
    pub fn read(&self, range: impl Into<BytesRange>) -> Result<Buffer> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "reader has been dropped"))?;
        self.handle.block_on(inner.read(range))
    }

    ///
    /// This operation will copy and write bytes into given [`BufMut`]. Allocation happens while
    /// [`BufMut`] doesn't have enough space.
    ///
    /// # Notes
    ///
    /// - Returning length smaller than range means we have reached the end of file.
    pub fn read_into(&self, buf: &mut impl BufMut, range: impl Into<BytesRange>) -> Result<usize> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "reader has been dropped"))?;
        self.handle.block_on(inner.read_into(buf, range))
    }

    /// Create a buffer iterator to read specific range from given reader.
    pub fn into_iterator(mut self, range: impl Into<BytesRange>) -> Result<BufferIterator> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "reader has been dropped"))?;
        let iter = self.handle.block_on(inner.into_stream(range))?;

        Ok(BufferIterator::new(self.handle.clone(), iter))
    }

    /// Convert reader into [`StdReader`] which implements [`futures::AsyncRead`],
    /// [`futures::AsyncSeek`] and [`futures::AsyncBufRead`].
    #[inline]
    pub fn into_std_read(mut self, range: impl Into<BytesRange>) -> Result<StdReader> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "reader has been dropped"))?;

        let r = self.handle.block_on(inner.into_futures_async_read(range))?;

        Ok(StdReader::new(self.handle.clone(), r))
    }

    /// Convert reader into [`StdBytesIterator`] which implements [`Iterator`].
    #[inline]
    pub fn into_bytes_iterator(mut self, range: impl Into<BytesRange>) -> Result<StdBytesIterator> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "reader has been dropped"))?;

        let iter = self.handle.block_on(inner.into_bytes_stream(range))?;
        Ok(StdBytesIterator::new(self.handle.clone(), iter))
    }
}

/// Make sure the inner reader is dropped in async context.
impl Drop for Reader {
    fn drop(&mut self) {
        if let Some(v) = self.inner.take() {
            self.handle.block_on(async move { drop(v) });
        }
    }
}
