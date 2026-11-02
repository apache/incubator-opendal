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

use serde::Deserialize;
use serde::Serialize;

use super::backend::GhacBuilder;

/// Config for GitHub Action Cache Services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GhacConfig {
    /// The root path for ghac.
    pub root: Option<String>,
    /// The version that used by cache.
    pub version: Option<String>,
    /// The endpoint for ghac service.
    pub endpoint: Option<String>,
    /// The runtime token for ghac service.
    pub runtime_token: Option<String>,
}

impl Debug for GhacConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GhacConfig")
            .field("root", &self.root)
            .field("version", &self.version)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for GhacConfig {
    type Builder = GhacBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), format!("https://{authority}"));
        }

        if let Some(path) = uri.root() {
            if map.contains_key("version") {
                if !path.is_empty() {
                    map.insert("root".to_string(), path.to_string());
                }
            } else if let Some((version, rest)) = path.split_once('/') {
                if !version.is_empty() {
                    map.insert("version".to_string(), version.to_string());
                }
                if !rest.is_empty() {
                    map.insert("root".to_string(), rest.to_string());
                }
            } else if !path.is_empty() {
                map.insert("version".to_string(), path.to_string());
            }
        }

        Self::from_iter(map)
    }

    #[allow(deprecated)]
    fn into_builder(self) -> Self::Builder {
        GhacBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_endpoint_version_and_root() {
        let uri = OperatorUri::new(
            "ghac://cache.githubactions.io/v1/cache-prefix",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = GhacConfig::from_uri(&uri).unwrap();
        assert_eq!(
            cfg.endpoint.as_deref(),
            Some("https://cache.githubactions.io")
        );
        assert_eq!(cfg.version.as_deref(), Some("v1"));
        assert_eq!(cfg.root.as_deref(), Some("cache-prefix"));
    }

    #[test]
    fn from_uri_respects_version_override() {
        let uri = OperatorUri::new(
            "ghac://cache.githubactions.io/cache-prefix",
            vec![("version".to_string(), "v2".to_string())],
        )
        .unwrap();

        let cfg = GhacConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.version.as_deref(), Some("v2"));
        assert_eq!(cfg.root.as_deref(), Some("cache-prefix"));
    }
}
