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

use std::collections::Bound;
use std::ops::Range;
use std::ops::RangeBounds;
use std::sync::Arc;

use bytes::BufMut;

use crate::raw::*;
use crate::*;

use super::buffer_iterator::BufferIterator;

/// BlockingReader is designed to read data from given path in an blocking
/// manner.
pub struct BlockingReader {
    pub(crate) inner: oio::BlockingReader,
    options: OpReader,
}

impl BlockingReader {
    /// Create a new blocking reader.
    ///
    /// Create will use internal information to decide the most suitable
    /// implementation for users.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) fn create(
        acc: Accessor,
        path: &str,
        op: OpRead,
        options: OpReader,
    ) -> crate::Result<Self> {
        let (_, r) = acc.blocking_read(path, op)?;

        Ok(BlockingReader {
            inner: Arc::new(r),
            options,
        })
    }

    /// Read give range from reader into [`Buffer`].
    ///
    /// This operation is zero-copy, which means it keeps the [`Bytes`] returned by underlying
    /// storage services without any extra copy or intensive memory allocations.
    ///
    /// # Notes
    ///
    /// - Buffer length smaller than range means we have reached the end of file.
    pub fn read(&self, range: impl RangeBounds<u64>) -> Result<Buffer> {
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

        let iter = BufferIterator::new(self.inner.clone(), self.options.clone(), start, end);

        Ok(iter
            .collect::<Result<Vec<Buffer>>>()?
            .into_iter()
            .flatten()
            .collect())
    }

    ///
    /// This operation will copy and write bytes into given [`BufMut`]. Allocation happens while
    /// [`BufMut`] doesn't have enough space.
    ///
    /// # Notes
    ///
    /// - Returning length smaller than range means we have reached the end of file.
    pub fn read_into(&self, buf: &mut impl BufMut, range: impl RangeBounds<u64>) -> Result<usize> {
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

        let iter = BufferIterator::new(self.inner.clone(), self.options.clone(), start, end);

        let bufs: Result<Vec<Buffer>> = iter.collect();

        let mut read = 0;
        for bs in bufs? {
            read += bs.len();
            buf.put(bs);
        }

        Ok(read)
    }

    /// Convert reader into [`StdReader`] which implements [`futures::AsyncRead`],
    /// [`futures::AsyncSeek`] and [`futures::AsyncBufRead`].
    #[inline]
    pub fn into_std_read(self, range: Range<u64>) -> StdReader {
        // TODO: the capacity should be decided by services.
        StdReader::new(self.inner.clone(), range)
    }

    /// Convert reader into [`StdBytesIterator`] which implements [`Iterator`].
    #[inline]
    pub fn into_bytes_iterator(self, range: Range<u64>) -> StdBytesIterator {
        StdBytesIterator::new(self.inner.clone(), range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::ThreadRng;
    use rand::Rng;
    use rand::RngCore;

    fn gen_random_bytes() -> Vec<u8> {
        let mut rng = ThreadRng::default();
        // Generate size between 1B..16MB.
        let size = rng.gen_range(1..16 * 1024 * 1024);
        let mut content = vec![0; size];
        rng.fill_bytes(&mut content);
        content
    }

    #[test]
    fn test_blocking_reader_read() {
        let op = Operator::new(services::Memory::default())
            .unwrap()
            .finish()
            .blocking();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone()).expect("write must succeed");

        let reader = op.reader(path).unwrap();
        let buf = reader.read(..).expect("read to end must succeed");

        assert_eq!(buf.to_bytes(), content);
    }

    #[test]
    fn test_reader_read_with_chunk() {
        let op = Operator::new(services::Memory::default())
            .unwrap()
            .finish()
            .blocking();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone()).expect("write must succeed");

        let reader = op.reader_with(path).chunk(16).call().unwrap();
        let buf = reader.read(..).expect("read to end must succeed");

        assert_eq!(buf.to_bytes(), content);
    }

    #[test]
    fn test_reader_read_with_concurrent() {
        let op = Operator::new(services::Memory::default())
            .unwrap()
            .finish()
            .blocking();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone()).expect("write must succeed");

        let reader = op
            .reader_with(path)
            .chunk(128)
            .concurrent(16)
            .call()
            .unwrap();
        let buf = reader.read(..).expect("read to end must succeed");

        assert_eq!(buf.to_bytes(), content);
    }

    #[test]
    fn test_reader_read_into() {
        let op = Operator::new(services::Memory::default())
            .unwrap()
            .finish()
            .blocking();
        let path = "test_file";

        let content = gen_random_bytes();
        op.write(path, content.clone()).expect("write must succeed");

        let reader = op.reader(path).unwrap();
        let mut buf = Vec::new();
        reader
            .read_into(&mut buf, ..)
            .expect("read to end must succeed");

        assert_eq!(buf, content);
    }
}
