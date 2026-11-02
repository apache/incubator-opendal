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

use opendal_core::Result;
use opendal_core::raw::OpDelete;
use opendal_core::raw::oio;

use crate::FoyerKey;
use crate::Inner;

#[doc(hidden)]
pub struct Deleter<D> {
    pub(crate) deleter: D,
    pub(crate) keys: Vec<FoyerKey>,
    pub(crate) inner: Arc<Inner>,
}

impl<D> Deleter<D> {
    pub(crate) fn new(deleter: D, inner: Arc<Inner>) -> Self {
        Self {
            deleter,
            keys: vec![],
            inner,
        }
    }
}

impl<D: oio::Delete> oio::Delete for Deleter<D> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.deleter.delete(path, args.clone()).await?;
        self.keys.push(FoyerKey {
            path: path.to_string(),
            version: args.version().map(|v| v.to_string()),
        });
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        {
            let mut deleted_keys = self
                .inner
                .deleted_keys
                .lock()
                .expect("deleted keys lock poisoned");
            for key in &self.keys {
                self.inner.cache.remove(key);
                deleted_keys.insert(key.clone());
            }
        }
        self.deleter.close().await
    }
}
