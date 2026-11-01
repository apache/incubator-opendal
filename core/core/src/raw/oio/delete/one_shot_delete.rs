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

use crate::raw::*;
use crate::*;

/// OneShotDelete is used to implement [`oio::Delete`] based on one shot operation.
///
/// OneShotDeleter will perform delete operation while calling `close`.
pub trait OneShotDelete: Send + Sync + Unpin + 'static {
    /// delete_once delete one path at once.
    ///
    /// Implementations should make sure that the data is deleted correctly at once.
    fn delete_once(
        &self,
        path: String,
        args: OpDelete,
    ) -> impl Future<Output = Result<()>> + MaybeSend;
}

/// OneShotDelete is used to implement [`oio::Delete`] based on one shot.
pub struct OneShotDeleter<D> {
    inner: D,
}

impl<D> OneShotDeleter<D> {
    /// Create a new one shot deleter.
    pub fn new(inner: D) -> Self {
        Self { inner }
    }
}

impl<D: OneShotDelete> oio::Delete for OneShotDeleter<D> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete_once(path.to_string(), args).await
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
