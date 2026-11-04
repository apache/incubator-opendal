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
use std::io::BufRead;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;

use futures::AsyncBufReadExt;
use futures::AsyncReadExt;
use futures::AsyncSeekExt;

use crate::*;

/// `StdReader` adapts a [`crate::blocking::Reader`] to [`Read`], [`Seek`], and
/// [`BufRead`].
///
/// Users can use this adapter in cases where they need to use [`Read`] or [`BufRead`] trait.
///
/// StdReader also implements [`Send`] and [`Sync`].
pub struct StdReader {
    handle: tokio::runtime::Handle,
    r: Option<FuturesAsyncReader>,
}

impl StdReader {
    /// NOTE: don't allow users to create StdReader directly.
    #[inline]
    pub(super) fn new(handle: tokio::runtime::Handle, r: FuturesAsyncReader) -> Self {
        Self { handle, r: Some(r) }
    }

    /// Read at most `size` bytes and return them as an OpenDAL [`Buffer`].
    ///
    /// This method preserves the underlying buffer storage and does not copy
    /// the payload. It can return fewer bytes than requested, including an
    /// empty buffer at EOF.
    pub fn read_buffer(&mut self, size: usize) -> io::Result<Buffer> {
        let Some(r) = self.r.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "reader has been dropped").into());
        };

        self.handle.block_on(r.read_buffer(size))
    }

    /// Return this reader's runtime handle.
    pub fn get_handle(&self) -> &tokio::runtime::Handle {
        &self.handle
    }

    /// Borrow the internal [`FuturesAsyncReader`].
    ///
    /// Read and seek operations update this reader's position and buffer.
    /// Use the runtime from [`Self::get_handle`] to run these operations.
    ///
    /// Return an error if the internal reader is unavailable.
    pub fn as_async_mut(&mut self) -> Result<&mut FuturesAsyncReader> {
        self.r
            .as_mut()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "reader has been dropped"))
    }

    /// Read all remaining bytes and return them as an OpenDAL [`Buffer`].
    ///
    /// This method preserves the underlying buffer storage and only allocates
    /// metadata when multiple buffers must be combined.
    pub fn read_to_end_buffer(&mut self) -> io::Result<Buffer> {
        let Some(r) = self.r.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "reader has been dropped").into());
        };

        self.handle.block_on(r.read_to_end_buffer())
    }
}

impl BufRead for StdReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let Some(r) = self.r.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "reader has been dropped").into());
        };

        self.handle.block_on(r.fill_buf())
    }

    fn consume(&mut self, amt: usize) {
        let Some(r) = self.r.as_mut() else {
            return;
        };

        r.consume_unpin(amt);
    }
}

impl Read for StdReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let Some(r) = self.r.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "reader has been dropped").into());
        };

        self.handle.block_on(r.read(buf))
    }
}

impl Seek for StdReader {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let Some(r) = self.r.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "reader has been dropped").into());
        };

        self.handle.block_on(r.seek(pos))
    }
}

/// Make sure the inner reader is dropped in async context.
impl Drop for StdReader {
    fn drop(&mut self) {
        if let Some(v) = self.r.take() {
            self.handle.block_on(async move { drop(v) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_async_mut_updates_original_reader() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let op =
            crate::blocking::Operator::new(Operator::new(services::Memory::default()).unwrap())
                .unwrap();
        op.write("source", "abcdef").unwrap();
        let mut reader = op.reader("source").unwrap().into_std_read(..).unwrap();
        let mut first = [0; 1];
        reader.read_exact(&mut first).unwrap();
        assert_eq!(&first, b"a");
        {
            let handle = reader.get_handle().clone();
            assert_eq!(handle.id(), runtime.handle().id());
            let inner = reader.as_async_mut().unwrap();
            handle.block_on(async {
                assert_eq!(inner.read_buffer(2).await.unwrap().to_bytes(), "bc");
                assert_eq!(inner.seek(SeekFrom::Current(1)).await.unwrap(), 4);
            });
        }
        assert_eq!(reader.stream_position().unwrap(), 4);
        let mut remaining = Vec::new();
        reader.read_to_end(&mut remaining).unwrap();
        assert_eq!(remaining, b"ef");
        reader.seek(SeekFrom::Start(1)).unwrap();
        {
            let handle = reader.get_handle().clone();
            let inner = reader.as_async_mut().unwrap();
            assert_eq!(
                handle.block_on(inner.read_buffer(1)).unwrap().to_bytes(),
                "b"
            );
        }
        assert_eq!(reader.stream_position().unwrap(), 2);
        reader.read_exact(&mut first).unwrap();
        assert_eq!(&first, b"c");
    }
}
