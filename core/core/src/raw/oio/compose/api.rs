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

/// Composer is a type-erased [`Compose`].
pub type Composer = Box<dyn ComposeDyn>;

/// Compose accepts ordered source objects and commits one destination object.
pub trait Compose: Unpin + Send + Sync {
    /// Accept a complete source object.
    ///
    /// A successful return means the source has been accepted in sequence.
    /// Backend work may still be pending.
    fn compose<'a>(
        &'a mut self,
        path: &'a str,
        args: OpRead,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a;

    /// Commit all accepted sources and return destination metadata.
    ///
    /// Return [`ErrorKind::ConfigInvalid`] if no source has been accepted.
    fn close(&mut self) -> impl Future<Output = Result<Metadata>> + MaybeSend;
}

impl Compose for () {
    async fn compose(&mut self, _: &str, _: OpRead) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output composer doesn't support compose",
        ))
    }

    async fn close(&mut self) -> Result<Metadata> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output composer doesn't support close",
        ))
    }
}

/// The dyn version of [`Compose`].
pub trait ComposeDyn: Unpin + Send + Sync {
    /// The dyn version of [`Compose::compose`].
    fn compose_dyn<'a>(&'a mut self, path: &'a str, args: OpRead) -> BoxedFuture<'a, Result<()>>;

    /// The dyn version of [`Compose::close`].
    fn close_dyn(&mut self) -> BoxedFuture<'_, Result<Metadata>>;
}

impl<T: Compose + ?Sized> ComposeDyn for T {
    fn compose_dyn<'a>(&'a mut self, path: &'a str, args: OpRead) -> BoxedFuture<'a, Result<()>> {
        Box::pin(Compose::compose(self, path, args))
    }

    fn close_dyn(&mut self) -> BoxedFuture<'_, Result<Metadata>> {
        Box::pin(self.close())
    }
}

impl<T: ComposeDyn + ?Sized> Compose for Box<T> {
    fn compose<'a>(
        &'a mut self,
        path: &'a str,
        args: OpRead,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a {
        self.deref_mut().compose_dyn(path, args)
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.deref_mut().close_dyn().await
    }
}
