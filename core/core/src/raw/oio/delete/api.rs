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

use crate::raw::BoxedFuture;
use crate::raw::MaybeSend;
use crate::raw::OpDelete;
use crate::*;

/// Deleter is a type erased [`Delete`].
pub type Deleter = Box<dyn DeleteDyn>;

/// The Delete trait defines interfaces for performing deletion operations.
///
/// Implementations may execute each request immediately or buffer requests for
/// batch deletion. Callers must always call [`Delete::close`] to flush pending
/// work before dropping the deleter.
pub trait Delete: Unpin + Send + Sync {
    /// Request deletion of a resource at the specified path with optional arguments.
    ///
    /// A successful return means the deleter has accepted the request. It does
    /// not guarantee that all backend work has completed.
    fn delete<'a>(
        &'a mut self,
        path: &'a str,
        args: OpDelete,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a;

    /// Flush pending requests and wait until all accepted deletions are complete.
    fn close(&mut self) -> impl Future<Output = Result<()>> + MaybeSend;
}

impl Delete for () {
    async fn delete(&mut self, _: &str, _: OpDelete) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output deleter doesn't support delete",
        ))
    }

    async fn close(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "output deleter doesn't support close",
        ))
    }
}

/// The dyn version of [`Delete`].
pub trait DeleteDyn: Unpin + Send + Sync {
    /// The dyn version of [`Delete::delete`].
    fn delete_dyn<'a>(&'a mut self, path: &'a str, args: OpDelete) -> BoxedFuture<'a, Result<()>>;

    /// The dyn version of [`Delete::close`].
    fn close_dyn(&mut self) -> BoxedFuture<'_, Result<()>>;
}

impl<T: Delete + ?Sized> DeleteDyn for T {
    fn delete_dyn<'a>(&'a mut self, path: &'a str, args: OpDelete) -> BoxedFuture<'a, Result<()>> {
        Box::pin(Delete::delete(self, path, args))
    }

    fn close_dyn(&mut self) -> BoxedFuture<'_, Result<()>> {
        Box::pin(self.close())
    }
}

impl<T: DeleteDyn + ?Sized> Delete for Box<T> {
    fn delete<'a>(
        &'a mut self,
        path: &'a str,
        args: OpDelete,
    ) -> impl Future<Output = Result<()>> + MaybeSend + 'a {
        self.deref_mut().delete_dyn(path, args)
    }

    async fn close(&mut self) -> Result<()> {
        self.deref_mut().close_dyn().await
    }
}
