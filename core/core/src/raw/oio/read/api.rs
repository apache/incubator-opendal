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

use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;

use bytes::Bytes;
use futures::Future;

use crate::raw::*;
use crate::*;

/// Reader is a type erased [`Read`].
pub type Reader = Box<dyn ReadDyn>;

/// Read is the internal trait used by OpenDAL to read ranges from storage.
///
/// Users should not use or import this trait unless they are implementing a `Service`.
///
/// This trait returns `impl Future`, so it is not object safe. Use [`ReadDyn`] when
/// type erasure is required.
pub trait Read: Unpin + Send + Sync {
    /// Open a range stream for the given range.
    fn open(
        &self,
        range: BytesRange,
    ) -> impl Future<Output = Result<(RpRead, Box<dyn ReadStreamDyn>)>> + MaybeSend;

    /// Read an exact bounded range into [`Buffer`].
    fn read(&self, range: BytesRange)
    -> impl Future<Output = Result<(RpRead, Buffer)>> + MaybeSend;
}

impl Read for () {
    async fn open(&self, _: BytesRange) -> Result<(RpRead, Box<dyn ReadStreamDyn>)> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output reader doesn't support open",
        ))
    }

    async fn read(&self, _: BytesRange) -> Result<(RpRead, Buffer)> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output reader doesn't support read",
        ))
    }
}

/// ReadDyn is the dyn-compatible adapter for [`Read`].
///
/// It boxes returned futures to support `Box<dyn ReadDyn>`, adding one allocation
/// per call at the type-erasure boundary.
pub trait ReadDyn: Unpin + Send + Sync {
    /// The dyn version of [`Read::open`].
    fn open_dyn(
        &self,
        range: BytesRange,
    ) -> BoxedFuture<'_, Result<(RpRead, Box<dyn ReadStreamDyn>)>>;

    /// The dyn version of [`Read::read`].
    fn read_dyn(&self, range: BytesRange) -> BoxedFuture<'_, Result<(RpRead, Buffer)>>;
}

impl<T: Read + ?Sized> ReadDyn for T {
    fn open_dyn(
        &self,
        range: BytesRange,
    ) -> BoxedFuture<'_, Result<(RpRead, Box<dyn ReadStreamDyn>)>> {
        Box::pin(self.open(range))
    }

    fn read_dyn(&self, range: BytesRange) -> BoxedFuture<'_, Result<(RpRead, Buffer)>> {
        Box::pin(self.read(range))
    }
}

impl<T: ReadDyn + ?Sized> Read for Box<T> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn ReadStreamDyn>)> {
        self.deref().open_dyn(range).await
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        self.deref().read_dyn(range).await
    }
}

/// ReadStream is the internal trait used by OpenDAL to stream data from storage.
///
/// Users should not use or import this trait unless they are implementing a `Service`.
///
/// This trait returns `impl Future`, so it is not object safe. Use [`ReadStreamDyn`]
/// when type erasure is required.
pub trait ReadStream: Unpin + Send + Sync {
    /// Read the next data chunk from the stream.
    fn read(&mut self) -> impl Future<Output = Result<Buffer>> + MaybeSend;

    /// Read all data from the reader.
    fn read_all(&mut self) -> impl Future<Output = Result<Buffer>> + MaybeSend {
        async {
            let mut bufs = vec![];
            loop {
                match self.read().await {
                    Ok(buf) if buf.is_empty() => break,
                    Ok(buf) => bufs.push(buf),
                    Err(err) => return Err(err),
                }
            }
            Ok(bufs.into_iter().flatten().collect())
        }
    }
}

impl ReadStream for () {
    async fn read(&mut self) -> Result<Buffer> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output reader doesn't support read",
        ))
    }
}

impl ReadStream for Bytes {
    async fn read(&mut self) -> Result<Buffer> {
        Ok(Buffer::from(self.split_off(0)))
    }
}

impl ReadStream for Buffer {
    async fn read(&mut self) -> Result<Buffer> {
        Ok(mem::take(self))
    }
}

/// ReadStreamDyn is the dyn-compatible adapter for [`ReadStream`].
///
/// It boxes returned futures to support `Box<dyn ReadStreamDyn>`, adding one
/// allocation per call at the type-erasure boundary.
pub trait ReadStreamDyn: Unpin + Send + Sync {
    /// The dyn version of [`ReadStream::read`].
    fn read_dyn(&mut self) -> BoxedFuture<'_, Result<Buffer>>;

    /// The dyn version of [`ReadStream::read_all`].
    fn read_all_dyn(&mut self) -> BoxedFuture<'_, Result<Buffer>>;
}

impl<T: ReadStream + ?Sized> ReadStreamDyn for T {
    fn read_dyn(&mut self) -> BoxedFuture<'_, Result<Buffer>> {
        Box::pin(self.read())
    }

    fn read_all_dyn(&mut self) -> BoxedFuture<'_, Result<Buffer>> {
        Box::pin(self.read_all())
    }
}

/// # NOTE
///
/// Take care about the `deref_mut()` here. This makes sure that we are calling functions
/// upon `&mut T` instead of `&mut Box<T>`. The later could result in infinite recursion.
impl<T: ReadStreamDyn + ?Sized> ReadStream for Box<T> {
    async fn read(&mut self) -> Result<Buffer> {
        self.deref_mut().read_dyn().await
    }

    async fn read_all(&mut self) -> Result<Buffer> {
        self.deref_mut().read_all_dyn().await
    }
}
