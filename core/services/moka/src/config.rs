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

use super::backend::MokaBuilder;
use opendal_core::raw::SignedDuration;
use opendal_core::{Configurator, OperatorUri, Result};

/// Config for Moka services support.
#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct MokaConfig {
    /// Name for this cache instance.
    pub name: Option<String>,
    /// Sets the max capacity of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::max_capacity`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.max_capacity)
    pub max_capacity: Option<u64>,
    /// Sets the time to live of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::time_to_live`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.time_to_live)
    pub time_to_live: Option<SignedDuration>,
    /// Sets the time to idle of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::time_to_idle`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.time_to_idle)
    pub time_to_idle: Option<SignedDuration>,

    /// root path of this backend
    pub root: Option<String>,
}

impl Configurator for MokaConfig {
    type Builder = MokaBuilder;

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
        MokaBuilder {
            config: self,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_name_and_root() {
        let uri =
            OperatorUri::new("moka:///cache?name=session", Vec::<(String, String)>::new()).unwrap();

        let cfg = MokaConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.name.as_deref(), Some("session"));
        assert_eq!(cfg.root.as_deref(), Some("cache"));
    }

    #[test]
    fn from_iter_parses_cache_durations() -> Result<()> {
        let cfg = MokaConfig::from_iter([
            ("time_to_live".to_string(), "1500ms".to_string()),
            ("time_to_idle".to_string(), "2s".to_string()),
        ])?;

        assert_eq!(cfg.time_to_live, Some(SignedDuration::from_millis(1500)));
        assert_eq!(cfg.time_to_idle, Some(SignedDuration::from_secs(2)));
        Ok(())
    }
}
