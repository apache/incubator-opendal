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

use mini_moka::sync::Cache;
use opendal_core::*;

/// Value stored in mini-moka cache containing both metadata and content
#[derive(Clone)]
pub struct MiniMokaValue {
    /// Stored metadata in mini-moka cache.
    pub metadata: Metadata,
    /// Stored content in mini-moka cache.
    pub content: Buffer,
}

#[derive(Clone)]
pub struct MiniMokaCore {
    pub cache: Cache<String, MiniMokaValue>,
}

impl Debug for MiniMokaCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiniMokaCore")
            .field("size", &self.cache.weighted_size())
            .field("count", &self.cache.entry_count())
            .finish()
    }
}

impl MiniMokaCore {
    pub fn get(&self, key: &str) -> Option<MiniMokaValue> {
        self.cache.get(&key.to_string())
    }
}
