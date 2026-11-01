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

use std::collections::HashSet;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use foyer::BlockEngineConfig;
use foyer::DeviceBuilder;
use foyer::FsDeviceBuilder;
use foyer::HybridCache;
use foyer::HybridCacheBuilder;
use foyer::RecoverMode;

use opendal_core::Buffer;
use opendal_core::Error;
use opendal_core::ErrorKind;
use opendal_core::Result;

use super::FoyerConfig;
use super::FoyerKey;
use super::FoyerValue;

/// FoyerCore holds the foyer HybridCache instance.
///
/// It supports lazy initialization when constructed from URI.
#[derive(Clone)]
pub struct FoyerCore {
    /// OnceLock for lazy cache initialization.
    cache: std::sync::OnceLock<Arc<HybridCache<FoyerKey, FoyerValue>>>,
    deleted_keys: Arc<Mutex<HashSet<FoyerKey>>>,
    /// Config
    config: FoyerConfig,
}

impl Debug for FoyerCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoyerCore")
            .field("name", &self.config.name)
            .finish_non_exhaustive()
    }
}

const FOYER_DEFAULT_MEMORY_BYTES: usize = 1024 * 1024 * 1024;
const FOYER_DEFAULT_DISK_FILE_SIZE: usize = 1024 * 1024;
const FOYER_DEFAULT_SHARDS: usize = 1;

fn parse_recover_mode(mode_str: &str) -> Result<RecoverMode> {
    match mode_str.to_lowercase().as_str() {
        "none" => Ok(RecoverMode::None),
        "quiet" => Ok(RecoverMode::Quiet),
        "strict" => Ok(RecoverMode::Strict),
        _ => Err(Error::new(
            ErrorKind::ConfigInvalid,
            format!(
                "invalid recover_mode: {}, expected 'none', 'quiet', or 'strict'",
                mode_str
            ),
        )),
    }
}

impl FoyerCore {
    /// Create a new FoyerCore with a pre-built cache.
    ///
    /// This is used when the user provides a `HybridCache` via `Foyer::new(cache)`.
    pub fn new(config: FoyerConfig) -> Self {
        Self {
            cache: std::sync::OnceLock::new(),
            deleted_keys: Arc::new(Mutex::new(HashSet::new())),
            config,
        }
    }

    /// Initialize the cache with the given pre-built instance.
    pub(crate) fn with_cache(self, cache: Arc<HybridCache<FoyerKey, FoyerValue>>) -> Self {
        let _ = self.cache.set(cache);
        self
    }

    /// Get the cache, initializing lazily if needed.
    async fn get_cache(&self) -> Result<Arc<HybridCache<FoyerKey, FoyerValue>>> {
        if let Some(cache) = self.cache.get() {
            return Ok(cache.clone());
        }

        let memory = self.config.memory.unwrap_or(FOYER_DEFAULT_MEMORY_BYTES);
        let shards = self.config.shards.unwrap_or(FOYER_DEFAULT_SHARDS);
        let recover_mode = if let Some(ref mode_str) = self.config.recover_mode {
            parse_recover_mode(mode_str)?
        } else {
            RecoverMode::None
        };

        let mut builder = HybridCacheBuilder::new()
            .memory(memory)
            .with_shards(shards)
            .storage()
            .with_recover_mode(recover_mode);

        // Configure disk storage if disk_path is provided
        if let Some(ref disk_path) = self.config.disk_path {
            let path = PathBuf::from(disk_path);

            let mut device = FsDeviceBuilder::new(path);

            if let Some(capacity) = self.config.disk_capacity {
                device = device.with_capacity(capacity);
            }

            let block_size = self
                .config
                .disk_file_size
                .unwrap_or(FOYER_DEFAULT_DISK_FILE_SIZE);
            let device = device.build().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "failed to build foyer cache device")
                    .set_source(e)
            })?;

            builder = builder
                .with_engine_config(BlockEngineConfig::new(device).with_block_size(block_size));
        }

        let cache = Arc::new(builder.build().await.map_err(|e| {
            Error::new(ErrorKind::Unexpected, "failed to build foyer cache").set_source(e)
        })?);

        if self.cache.set(cache.clone()).is_err() {
            return self.cache.get().cloned().ok_or_else(|| {
                Error::new(ErrorKind::Unexpected, "failed to initialize foyer cache")
            });
        }

        Ok(cache)
    }

    pub fn name(&self) -> Option<&str> {
        self.config.name.as_deref()
    }

    pub async fn get(&self, key: &str) -> Result<Option<Buffer>> {
        let cache = self.get_cache().await?;
        let foyer_key = FoyerKey {
            path: key.to_string(),
        };
        if self
            .deleted_keys
            .lock()
            .expect("deleted keys lock poisoned")
            .contains(&foyer_key)
        {
            return Ok(None);
        }

        match cache.get(&foyer_key).await {
            Ok(Some(entry)) => Ok(Some(entry.value().0.clone())),
            Ok(None) => Ok(None),
            Err(_) => Ok(None),
        }
    }

    pub async fn insert(&self, key: &str, value: Buffer) -> Result<()> {
        let cache = self.get_cache().await?;
        let key = FoyerKey {
            path: key.to_string(),
        };
        cache.insert(key.clone(), FoyerValue(value));
        self.deleted_keys
            .lock()
            .expect("deleted keys lock poisoned")
            .remove(&key);
        Ok(())
    }

    pub async fn remove(&self, key: &str) -> Result<()> {
        let cache = self.get_cache().await?;
        let key = FoyerKey {
            path: key.to_string(),
        };
        cache.remove(&key);
        self.deleted_keys
            .lock()
            .expect("deleted keys lock poisoned")
            .insert(key);
        Ok(())
    }
}
