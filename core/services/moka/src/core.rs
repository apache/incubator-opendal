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

use std::fmt::Debug;

use moka::future::Cache;

use opendal_core::*;

/// Value stored in moka cache containing both metadata and content
#[derive(Clone)]
pub struct MokaValue {
    /// Stored metadata in moka cache.
    pub metadata: Metadata,
    /// Stored content in moka cache.
    pub content: Buffer,
}

#[derive(Clone)]
pub struct MokaCore {
    pub cache: Cache<String, MokaValue>,
}

impl Debug for MokaCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MokaCore")
            .field("size", &self.cache.weighted_size())
            .field("count", &self.cache.entry_count())
            .finish()
    }
}

impl MokaCore {
    pub async fn get(&self, key: &str) -> Result<Option<MokaValue>> {
        Ok(self.cache.get(key).await)
    }

    pub async fn set(&self, key: &str, value: MokaValue) -> Result<()> {
        self.cache.insert(key.to_string(), value).await;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        self.cache.invalidate(key).await;
        Ok(())
    }
}
