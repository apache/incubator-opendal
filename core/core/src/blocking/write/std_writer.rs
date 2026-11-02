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

use std::io::Write;

use futures::AsyncWriteExt;

use crate::*;

/// `StdWriter` adapts a [`crate::blocking::Writer`] to [`std::io::Write`].
///
/// Users can use this adapter in cases where they need to use [`std::io::Write`] related trait.
///
/// # Notes
///
/// Files are automatically closed when they go out of scope. Errors detected on closing are ignored
/// by the implementation of Drop. Use the method `close` if these errors must be manually handled.
pub struct StdWriter {
    handle: tokio::runtime::Handle,
    w: Option<FuturesAsyncWriter>,
}

impl StdWriter {
    /// NOTE: don't allow users to create directly.
    #[inline]
    pub(crate) fn new(handle: tokio::runtime::Handle, w: Writer) -> Self {
        StdWriter {
            handle,
            w: Some(w.into_futures_async_write()),
        }
    }

    /// Close the internal writer and make sure all data have been stored.
    pub fn close(&mut self) -> std::io::Result<()> {
        let Some(w) = self.w.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "writer has been dropped").into());
        };

        self.handle.block_on(w.close())
    }
}

impl Write for StdWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let Some(w) = self.w.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "writer has been dropped").into());
        };

        self.handle.block_on(w.write(buf))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let Some(w) = self.w.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "writer has been dropped").into());
        };

        self.handle.block_on(w.flush())
    }
}

/// Make sure the inner writer is dropped in async context.
impl Drop for StdWriter {
    fn drop(&mut self) {
        if let Some(v) = self.w.take() {
            self.handle.block_on(async move { drop(v) });
        }
    }
}
