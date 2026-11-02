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
