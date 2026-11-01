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

use bytes::Bytes;
use futures::StreamExt;

use crate::*;

/// `StdBytesIterator` adapts a [`crate::blocking::Reader`] into an
/// [`Iterator`].
///
/// Users can use this adapter in cases where they need to use [`Iterator`] trait.
///
/// StdIterator also implements [`Send`] and [`Sync`].
pub struct StdBytesIterator {
    handle: tokio::runtime::Handle,
    inner: Option<FuturesBytesStream>,
}

impl StdBytesIterator {
    /// NOTE: don't allow users to create StdIterator directly.
    #[inline]
    pub(crate) fn new(handle: tokio::runtime::Handle, inner: FuturesBytesStream) -> Self {
        StdBytesIterator {
            handle,
            inner: Some(inner),
        }
    }
}

impl Iterator for StdBytesIterator {
    type Item = io::Result<Bytes>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(inner) = self.inner.as_mut() else {
            return Some(Err(Error::new(
                ErrorKind::Unexpected,
                "reader has been dropped",
            )
            .into()));
        };

        self.handle.block_on(inner.next())
    }
}

/// Make sure the inner reader is dropped in async context.
impl Drop for StdBytesIterator {
    fn drop(&mut self) {
        if let Some(v) = self.inner.take() {
            self.handle.block_on(async move { drop(v) });
        }
    }
}
