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

use serde::Deserialize;
use serde::Serialize;

use super::backend::FoyerBuilder;
use opendal_core::{Configurator, OperatorUri, Result};

/// Config for Foyer services support.
#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct FoyerConfig {
    /// Name for this cache instance.
    pub name: Option<String>,
    /// Root path of this backend.
    pub root: Option<String>,
    /// Memory capacity in bytes for the cache.
    pub memory: Option<usize>,
    /// Disk cache directory path.
    ///
    /// If set, enables hybrid cache with disk storage. Data will be persisted to
    /// this directory when memory cache is full.
    pub disk_path: Option<String>,
    /// Disk cache total capacity in bytes.
    /// Only used when `disk_path` is set.
    pub disk_capacity: Option<usize>,
    /// Individual cache file size in bytes.
    ///
    /// Default is 1 MiB.
    /// Only used when `disk_path` is set.
    pub disk_file_size: Option<usize>,
    /// Recovery mode when starting the cache.
    ///
    /// Valid values: "none" (default), "quiet", "strict".
    /// - "none": Don't recover from disk
    /// - "quiet": Recover and skip errors
    /// - "strict": Recover and panic on errors
    pub recover_mode: Option<String>,
    /// Number of shards for concurrent access.
    ///
    /// Default is 1. Higher values improve concurrency but increase overhead.
    pub shards: Option<usize>,
}

impl Configurator for FoyerConfig {
    type Builder = FoyerBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
        let mut map = uri.options().clone();

        if let Some(name) = uri.option("name") {
            map.insert("name".to_string(), name.to_string());
        }

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        FoyerBuilder {
            config: self,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_uri_sets_memory() {
        let uri = OperatorUri::new(
            "foyer:///cache?name=test&memory=67108864",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = FoyerConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.name.as_deref(), Some("test"));
        assert_eq!(cfg.root.as_deref(), Some("cache"));
        assert_eq!(cfg.memory, Some(64 * 1024 * 1024));
    }

    #[test]
    fn test_from_uri_sets_name_and_root() {
        let uri =
            OperatorUri::new("foyer:///data?name=session", Vec::<(String, String)>::new()).unwrap();

        let cfg = FoyerConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.name.as_deref(), Some("session"));
        assert_eq!(cfg.root.as_deref(), Some("data"));
    }

    #[test]
    fn test_from_uri_sets_disk_config() {
        let uri = OperatorUri::new(
            "foyer:///cache?memory=67108864&disk_path=/tmp/foyer&disk_capacity=1073741824&disk_file_size=2097152",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = FoyerConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.memory, Some(64 * 1024 * 1024));
        assert_eq!(cfg.disk_path.as_deref(), Some("/tmp/foyer"));
        assert_eq!(cfg.disk_capacity, Some(1024 * 1024 * 1024));
        assert_eq!(cfg.disk_file_size, Some(2 * 1024 * 1024));
    }

    #[test]
    fn test_from_uri_sets_recovery_and_shards() {
        let uri = OperatorUri::new(
            "foyer:///?recover_mode=quiet&shards=4",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = FoyerConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.recover_mode.as_deref(), Some("quiet"));
        assert_eq!(cfg.shards, Some(4));
    }
}
