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

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use crate::*;

/// Value stored in memory containing both metadata and content
#[derive(Clone)]
pub struct MemoryValue {
    pub metadata: Metadata,
    pub content: Buffer,
}

#[derive(Clone)]
pub struct MemoryCore {
    pub data: Arc<Mutex<BTreeMap<String, MemoryValue>>>,
}

impl Debug for MemoryCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryCore").finish_non_exhaustive()
    }
}

impl MemoryCore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn get(&self, key: &str) -> Result<Option<MemoryValue>> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }

    pub fn set(&self, key: &str, value: MemoryValue) -> Result<()> {
        self.data.lock().unwrap().insert(key.to_string(), value);
        Ok(())
    }

    pub fn set_if_not_exists(&self, key: &str, value: MemoryValue) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        match data.entry(key.to_string()) {
            Entry::Vacant(entry) => entry.insert(value),
            Entry::Occupied(_) => {
                return Err(Error::new(
                    ErrorKind::ConditionNotMatch,
                    "key already exists",
                ));
            }
        };
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }

    pub fn scan(&self, prefix: &str) -> Result<Vec<String>> {
        let data = self.data.lock().unwrap();

        if prefix.is_empty() {
            return Ok(data.keys().cloned().collect());
        }

        let mut keys = Vec::new();
        for (key, _) in data.range(prefix.to_string()..) {
            if !key.starts_with(prefix) {
                break;
            }
            keys.push(key.clone());
        }
        Ok(keys)
    }
}
