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

use opendal_core::Configurator;
use opendal_core::OperatorUri;
use opendal_core::Result;
use opendal_core::raw::*;
use serde::Deserialize;
use serde::Serialize;

use super::backend::MemcachedBuilder;

/// Config for MemCached services support
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct MemcachedConfig {
    /// network address of the memcached service.
    ///
    /// For example: "tcp://localhost:11211"
    pub endpoint: Option<String>,
    /// the working directory of the service. Can be "/path/to/dir"
    ///
    /// default is "/"
    pub root: Option<String>,
    /// Memcached username, optional.
    pub username: Option<String>,
    /// Memcached password, optional.
    pub password: Option<String>,
    /// The default ttl for put operations.
    pub default_ttl: Option<SignedDuration>,
    /// The maximum number of connections allowed.
    ///
    /// default is 10
    pub connection_pool_max_size: Option<usize>,
}

impl Debug for MemcachedConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemcachedConfig")
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .field("username", &self.username)
            .field("default_ttl", &self.default_ttl)
            .finish_non_exhaustive()
    }
}

impl Configurator for MemcachedConfig {
    type Builder = MemcachedBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), format!("tcp://{authority}"));
        }

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        MemcachedBuilder {
            config: self,
            default_ttl: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_uri_sets_endpoint_and_root() -> Result<()> {
        let uri = OperatorUri::new(
            "memcached://cache.local:11211/app/session",
            Vec::<(String, String)>::new(),
        )?;

        let cfg = MemcachedConfig::from_uri(&uri)?;
        assert_eq!(cfg.endpoint.as_deref(), Some("tcp://cache.local:11211"));
        assert_eq!(cfg.root.as_deref(), Some("app/session"));
        Ok(())
    }

    #[test]
    fn from_iter_parses_default_ttl() -> Result<()> {
        let cfg = MemcachedConfig::from_iter([("default_ttl".to_string(), "1500ms".to_string())])?;

        assert_eq!(cfg.default_ttl, Some(SignedDuration::from_millis(1500)));
        Ok(())
    }
}
