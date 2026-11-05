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

use crate::FoyerKey;
use crate::Inner;

#[doc(hidden)]
pub struct Copier<C> {
    copier: C,
    to: String,
    inner: Arc<Inner>,
}

impl<C> Copier<C> {
    pub(crate) fn new(copier: C, inner: Arc<Inner>, to: &str) -> Self {
        Self {
            copier,
            to: to.to_string(),
            inner,
        }
    }
}

impl<C: oio::Copy> oio::Copy for Copier<C> {
    async fn next(&mut self) -> Result<Option<usize>> {
        self.copier.next().await
    }

    async fn close(&mut self) -> Result<Metadata> {
        let metadata = self.copier.close().await?;
        // The destination now holds new content: drop any stale cached entry
        // and clear a previous deleted mark, mirroring a successful write.
        // Only the version-less entry is invalidated, matching delete: entries
        // for specific versions name immutable versions and are left alone.
        let key = FoyerKey {
            path: self.to.clone(),
            version: None,
        };
        self.inner.cache.remove(&key);
        self.inner
            .deleted_keys
            .lock()
            .expect("deleted keys lock poisoned")
            .remove(&key);
        Ok(metadata)
    }

    async fn abort(&mut self) -> Result<()> {
        self.copier.abort().await
    }
}
