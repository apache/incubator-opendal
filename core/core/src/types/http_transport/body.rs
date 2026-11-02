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

use std::cmp::Ordering;

use futures::Stream;
use futures::StreamExt;

use crate::raw::oio;
use crate::raw::oio::ReadStream;
use crate::types::Buffer;
use crate::types::Error;
use crate::types::ErrorKind;
use crate::types::Result;

/// The streaming body returned by [`HttpTransporter`].
///
/// We implement [`oio::ReadStream`] for `HttpBody`. Services can use
/// `HttpBody` as [`Service::Reader`].
///
/// [`HttpTransporter`]: super::HttpTransporter
/// [`Service::Reader`]: crate::raw::Service::Reader
pub struct HttpBody {
    #[cfg(not(target_arch = "wasm32"))]
    stream: Box<dyn Stream<Item = Result<Buffer>> + Send + Sync + Unpin + 'static>,
    #[cfg(target_arch = "wasm32")]
    stream: Box<dyn Stream<Item = Result<Buffer>> + Unpin + 'static>,
    size: Option<u64>,
    consumed: u64,
}

/// # Safety
///
/// `HttpBody` is `Send` on non-wasm targets.
unsafe impl Send for HttpBody {}

/// # Safety
///
/// `HttpBody` is `Sync` on non-wasm targets.
unsafe impl Sync for HttpBody {}

impl HttpBody {
    /// Create a new `HttpBody` with given stream and optional size.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<S>(stream: S, size: Option<u64>) -> Self
    where
        S: Stream<Item = Result<Buffer>> + Send + Sync + Unpin + 'static,
    {
        HttpBody {
            stream: Box::new(stream),
            size,
            consumed: 0,
        }
    }

    /// Create a new `HttpBody` with given stream and optional size.
    #[cfg(target_arch = "wasm32")]
    pub fn new<S>(stream: S, size: Option<u64>) -> Self
    where
        S: Stream<Item = Result<Buffer>> + Unpin + 'static,
    {
        HttpBody {
            stream: Box::new(stream),
            size,
            consumed: 0,
        }
    }

    /// Map the inner stream.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn map_inner(
        mut self,
        f: impl FnOnce(
            Box<dyn Stream<Item = Result<Buffer>> + Send + Sync + Unpin + 'static>,
        )
            -> Box<dyn Stream<Item = Result<Buffer>> + Send + Sync + Unpin + 'static>,
    ) -> Self {
        self.stream = f(self.stream);
        self
    }

    /// Map the inner stream.
    #[cfg(target_arch = "wasm32")]
    pub fn map_inner(
        mut self,
        f: impl FnOnce(
            Box<dyn Stream<Item = Result<Buffer>> + Unpin + 'static>,
        ) -> Box<dyn Stream<Item = Result<Buffer>> + Unpin + 'static>,
    ) -> Self {
        self.stream = f(self.stream);
        self
    }

    /// Read all data from the stream.
    pub async fn to_buffer(&mut self) -> Result<Buffer> {
        self.read_all().await
    }

    #[inline]
    fn check(&self) -> Result<()> {
        let Some(expect) = self.size else {
            return Ok(());
        };

        let actual = self.consumed;
        match actual.cmp(&expect) {
            Ordering::Equal => Ok(()),
            Ordering::Less => Err(Error::new(
                ErrorKind::Unexpected,
                format!("http response got too little data, expect: {expect}, actual: {actual}"),
            )
            .set_temporary()),
            Ordering::Greater => Err(Error::new(
                ErrorKind::Unexpected,
                format!("http response got too much data, expect: {expect}, actual: {actual}"),
            )
            .set_temporary()),
        }
    }
}

impl oio::ReadStream for HttpBody {
    async fn read(&mut self) -> Result<Buffer> {
        match self.stream.next().await.transpose()? {
            Some(buf) => {
                self.consumed += buf.len() as u64;
                Ok(buf)
            }
            None => {
                self.check()?;
                Ok(Buffer::new())
            }
        }
    }
}
