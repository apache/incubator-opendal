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

use dashmap::DashMap;
use opendal_core::*;

/// Value stored in dashmap cache containing both metadata and content
#[derive(Clone)]
pub struct DashmapValue {
    /// Stored metadata in dashmap cache.
    pub metadata: Metadata,
    /// Stored content in dashmap cache.
    pub content: Buffer,
}

#[derive(Clone)]
pub struct DashmapCore {
    pub cache: DashMap<String, DashmapValue>,
}

impl Debug for DashmapCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DashmapCore")
            .field("size", &self.cache.len())
            .finish()
    }
}

impl DashmapCore {
    pub fn get(&self, key: &str) -> Result<Option<DashmapValue>> {
        Ok(self.cache.get(key).map(|v| v.value().clone()))
    }

    pub fn set(&self, key: &str, value: DashmapValue) -> Result<()> {
        self.cache.insert(key.to_string(), value);
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        self.cache.remove(key);
        Ok(())
    }
}
