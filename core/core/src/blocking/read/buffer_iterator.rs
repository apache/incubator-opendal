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

use futures::StreamExt;

use crate::Buffer;
use crate::*;

/// BufferIterator is an iterator of buffers.
///
/// # TODO
///
/// We can support chunked reader for concurrent read in the future.
pub struct BufferIterator {
    handle: tokio::runtime::Handle,
    inner: Option<BufferStream>,
}

impl BufferIterator {
    /// Create a new buffer iterator.
    #[inline]
    pub(crate) fn new(handle: tokio::runtime::Handle, inner: BufferStream) -> Self {
        Self {
            handle,
            inner: Some(inner),
        }
    }
}

impl Iterator for BufferIterator {
    type Item = Result<Buffer>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(inner) = self.inner.as_mut() else {
            return Some(Err(Error::new(
                ErrorKind::Unexpected,
                "reader has been dropped",
            )));
        };

        self.handle.block_on(inner.next())
    }
}

/// Make sure the inner reader is dropped in async context.
impl Drop for BufferIterator {
    fn drop(&mut self) {
        if let Some(v) = self.inner.take() {
            self.handle.block_on(async move { drop(v) });
        }
    }
}
