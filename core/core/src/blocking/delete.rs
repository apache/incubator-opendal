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

use crate::Deleter as AsyncDeleter;
use crate::*;

/// BlockingDeleter is designed to continuously remove content from storage.
///
/// It leverages batch deletion capabilities provided by storage services for efficient removal.
pub struct Deleter {
    handle: tokio::runtime::Handle,
    inner: AsyncDeleter,
}

impl Deleter {
    pub(crate) fn create(handle: tokio::runtime::Handle, inner: AsyncDeleter) -> Result<Self> {
        Ok(Self { handle, inner })
    }

    /// Delete a path with default or per-entry options.
    ///
    /// Pass a path directly to use [`options::DeleteOptions::default`]. Pass
    /// `(String, DeleteOptions)` when this entry needs version or condition
    /// options.
    pub fn delete(&mut self, input: impl IntoDeleteInput) -> Result<()> {
        self.handle.block_on(self.inner.delete(input))
    }

    /// Delete an infallible iterator of paths.
    ///
    /// Also see:
    ///
    /// - [`Deleter::delete_try_iter`]: delete a fallible iterator of paths.
    pub fn delete_iter<I, D>(&mut self, iter: I) -> Result<()>
    where
        I: IntoIterator<Item = D>,
        D: IntoDeleteInput,
    {
        self.handle.block_on(self.inner.delete_iter(iter))
    }

    /// Delete an fallible iterator of paths.
    ///
    /// Also see:
    ///
    /// - [`Deleter::delete_iter`]: delete an infallible iterator of paths.
    pub fn delete_try_iter<I, D>(&mut self, try_iter: I) -> Result<()>
    where
        I: IntoIterator<Item = Result<D>>,
        D: IntoDeleteInput,
    {
        self.handle.block_on(self.inner.delete_try_iter(try_iter))
    }

    /// Close the deleter, this will flush the deleter and wait until all paths are deleted.
    pub fn close(&mut self) -> Result<()> {
        self.handle.block_on(self.inner.close())
    }
}
