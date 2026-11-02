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

use futures::Future;

use crate::raw::oio::ReadStream;
use crate::raw::*;
use crate::*;

/// StreamRead is used to implement [`oio::Read`] based on native range streams.
///
/// Services that implement [`StreamRead`] only need to expose their native
/// `open(range)` operation. [`StreamReader`] will provide the complete
/// [`oio::Read`] contract.
pub trait StreamRead: Send + Sync + Unpin + 'static {
    /// Open a stream to read the requested range.
    fn open(
        &self,
        range: BytesRange,
    ) -> impl Future<Output = Result<(RpRead, Box<dyn oio::ReadStreamDyn>)>> + MaybeSend;
}

/// StreamReader implements [`oio::Read`] based on [`StreamRead`].
pub struct StreamReader<R: StreamRead> {
    inner: R,
}

impl<R: StreamRead> StreamReader<R> {
    /// Create a new [`StreamReader`].
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    /// Consume the reader and return the inner [`StreamRead`].
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: StreamRead> oio::Read for StreamReader<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        self.inner.open(range).await
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let expected = range
            .size()
            .ok_or_else(|| Error::new(ErrorKind::Unsupported, "read requires a bounded range"))?;

        let (rp, mut stream) = self.inner.open(range).await?;
        let buffer = stream.read_all().await?;
        if buffer.len() as u64 != expected {
            return Err(
                Error::new(ErrorKind::Unexpected, "reader got unexpected data size")
                    .with_context("expect", expected)
                    .with_context("actual", buffer.len() as u64),
            );
        }

        Ok((rp, buffer))
    }
}
