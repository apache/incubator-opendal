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

use super::std_writer::StdWriter;
use crate::Writer as AsyncWriter;
use crate::*;

/// BlockingWriter is designed to write data into given path in an blocking
/// manner.
pub struct Writer {
    handle: tokio::runtime::Handle,
    inner: Option<AsyncWriter>,
}

impl Writer {
    /// Create a new writer.
    ///
    /// Create will use internal information to decide the most suitable
    /// implementation for users.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) fn new(handle: tokio::runtime::Handle, inner: AsyncWriter) -> Self {
        Self {
            handle,
            inner: Some(inner),
        }
    }

    /// Write [`Buffer`] into writer.
    ///
    /// This operation will write all data in given buffer into writer.
    ///
    /// ## Examples
    ///
    /// ```
    /// use bytes::Bytes;
    /// use opendal_core::blocking;
    /// use opendal_core::blocking::Operator;
    /// use opendal_core::Result;
    ///
    /// fn test(op: blocking::Operator) -> Result<()> {
    ///     let mut w = op.writer("hello.txt")?;
    ///     // Buffer can be created from continues bytes.
    ///     w.write("hello, world")?;
    ///     // Buffer can also be created from non-continues bytes.
    ///     w.write(vec![Bytes::from("hello,"), Bytes::from("world!")])?;
    ///
    ///     // Make sure file has been written completely.
    ///     w.close()?;
    ///     Ok(())
    /// }
    /// ```
    pub fn write(&mut self, bs: impl Into<Buffer>) -> Result<()> {
        let Some(inner) = self.inner.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "writer has been dropped"));
        };

        self.handle.block_on(inner.write(bs))
    }

    /// Close the writer and make sure all data have been committed.
    ///
    /// ## Notes
    ///
    /// Close should only be called when the writer is not closed or
    /// aborted, otherwise an unexpected error could be returned.
    pub fn close(&mut self) -> Result<Metadata> {
        let Some(inner) = self.inner.as_mut() else {
            return Err(Error::new(ErrorKind::Unexpected, "writer has been dropped"));
        };

        self.handle.block_on(inner.close())
    }

    /// Convert writer into [`StdWriter`] which implements [`std::io::Write`],
    pub fn into_std_write(mut self) -> StdWriter {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "writer has been dropped"))
            .expect("writer has been dropped");

        StdWriter::new(self.handle.clone(), inner)
    }
}

/// Make sure the inner writer is dropped in async context.
impl Drop for Writer {
    fn drop(&mut self) {
        if let Some(v) = self.inner.take() {
            self.handle.block_on(async move { drop(v) });
        }
    }
}
