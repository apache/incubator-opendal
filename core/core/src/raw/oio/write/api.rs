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

use std::future::Future;
use std::ops::DerefMut;

use crate::raw::*;
use crate::*;

/// Writer is a type-erased [`Write`].
pub type Writer = Box<dyn WriteDyn>;

/// Write is the async sink used by services and layers.
pub trait Write: Unpin + Send + Sync {
    /// Write the entire buffer into the writer.
    ///
    /// `Ok(())` means all bytes from `bs` have been accepted. Implementations
    /// must return an error instead of treating a partial write as success.
    fn write(&mut self, bs: Buffer) -> impl Future<Output = Result<()>> + MaybeSend;

    /// Copy one absolute bounded source range into this writer.
    ///
    /// Callers invoke this operation only when the composed service declares
    /// [`Capability::write_can_copy_from`]. Every error is an execution failure
    /// and must not trigger streaming fallback.
    fn copy_from(
        &mut self,
        _path: &str,
        _args: OpRead,
        _range: BytesRange,
    ) -> impl Future<Output = Result<()>> + MaybeSend {
        async {
            Err(Error::new(
                ErrorKind::Unsupported,
                "writer doesn't support native copy",
            ))
        }
    }

    /// Close the writer and make sure all data has been flushed.
    fn close(&mut self) -> impl Future<Output = Result<Metadata>> + MaybeSend;

    /// Abort the pending writer.
    fn abort(&mut self) -> impl Future<Output = Result<()>> + MaybeSend;
}

impl Write for () {
    async fn write(&mut self, _: Buffer) -> Result<()> {
        unimplemented!("write is required to be implemented for oio::Write")
    }

    async fn close(&mut self) -> Result<Metadata> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output writer doesn't support close",
        ))
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output writer doesn't support abort",
        ))
    }
}

/// WriteDyn is the object-safe version of [`Write`] used by [`Writer`].
pub trait WriteDyn: Unpin + Send + Sync {
    /// The dyn version of [`Write::write`].
    fn write_dyn(&mut self, bs: Buffer) -> BoxedFuture<'_, Result<()>>;

    /// The dyn version of [`Write::copy_from`].
    fn copy_from_dyn<'a>(
        &'a mut self,
        path: &'a str,
        args: OpRead,
        range: BytesRange,
    ) -> BoxedFuture<'a, Result<()>>;

    /// The dyn version of [`Write::close`].
    fn close_dyn(&mut self) -> BoxedFuture<'_, Result<Metadata>>;

    /// The dyn version of [`Write::abort`].
    fn abort_dyn(&mut self) -> BoxedFuture<'_, Result<()>>;
}

impl<T: Write + ?Sized> WriteDyn for T {
    fn write_dyn(&mut self, bs: Buffer) -> BoxedFuture<'_, Result<()>> {
        Box::pin(self.write(bs))
    }

    fn copy_from_dyn<'a>(
        &'a mut self,
        path: &'a str,
        args: OpRead,
        range: BytesRange,
    ) -> BoxedFuture<'a, Result<()>> {
        Box::pin(self.copy_from(path, args, range))
    }

    fn close_dyn(&mut self) -> BoxedFuture<'_, Result<Metadata>> {
        Box::pin(self.close())
    }

    fn abort_dyn(&mut self) -> BoxedFuture<'_, Result<()>> {
        Box::pin(self.abort())
    }
}

impl<T: WriteDyn + ?Sized> Write for Box<T> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.deref_mut().write_dyn(bs).await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        self.deref_mut().copy_from_dyn(path, args, range).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.deref_mut().close_dyn().await
    }

    async fn abort(&mut self) -> Result<()> {
        self.deref_mut().abort_dyn().await
    }
}
