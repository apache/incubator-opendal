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

use std::sync::Arc;

use bytes::Buf;

use crate::raw::*;
use crate::*;

/// BlockingWriter is designed to write data into given path in an blocking
/// manner.
pub struct BlockingWriter {
    /// Keep a reference to write context in writer.
    _ctx: Arc<WriteContext>,
    inner: WriteGenerator<oio::BlockingWriter>,
}

impl BlockingWriter {
    /// Create a new writer.
    ///
    /// Create will use internal information to decide the most suitable
    /// implementation for users.
    ///
    /// We don't want to expose those details to users so keep this function
    /// in crate only.
    pub(crate) fn new(ctx: WriteContext) -> Result<Self> {
        let ctx = Arc::new(ctx);
        let inner = WriteGenerator::blocking_create(ctx.clone())?;

        Ok(Self { _ctx: ctx, inner })
    }

    /// Write [`Buffer`] into writer.
    ///
    /// This operation will write all data in given buffer into writer.
    ///
    /// ## Examples
    ///
    /// ```
    /// use bytes::Bytes;
    /// use opendal::BlockingOperator;
    /// use opendal::Result;
    ///
    /// async fn test(op: BlockingOperator) -> Result<()> {
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
        let mut bs = bs.into();
        while !bs.is_empty() {
            let n = self.inner.write(bs.clone())?;
            bs.advance(n);
        }
        Ok(())
    }

    /// Close the writer and make sure all data have been committed.
    ///
    /// ## Notes
    ///
    /// Close should only be called when the writer is not closed or
    /// aborted, otherwise an unexpected error could be returned.
    pub fn close(&mut self) -> Result<Metadata> {
        self.inner.close()
    }

    /// Convert writer into [`StdWriter`] which implements [`std::io::Write`],
    pub fn into_std_write(self) -> StdWriter {
        StdWriter::new(self.inner)
    }
}
