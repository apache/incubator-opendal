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

use std::sync::Arc;

use opendal_core::Metadata;
use opendal_core::Result;
use opendal_core::raw::oio;

use crate::Inner;

/// A copier that drops the destination's cached entry once the copy lands.
///
/// The cached content of `to` is stale after a copy on top of it; without this
/// a read of `to` kept serving the old object (#8489).
#[doc(hidden)]
pub struct Copier<C> {
    pub(crate) copier: C,
    pub(crate) to: String,
    pub(crate) inner: Arc<Inner>,
}

impl<C> Copier<C> {
    pub(crate) fn new(copier: C, to: String, inner: Arc<Inner>) -> Self {
        Self { copier, to, inner }
    }
}

impl<C: oio::Copy> oio::Copy for Copier<C> {
    async fn next(&mut self) -> Result<Option<usize>> {
        self.copier.next().await
    }

    async fn close(&mut self) -> Result<Metadata> {
        let metadata = self.copier.close().await?;
        self.inner.invalidate(&self.to);
        Ok(metadata)
    }

    async fn abort(&mut self) -> Result<()> {
        self.copier.abort().await
    }
}
