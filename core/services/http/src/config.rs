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

use super::backend::HttpBuilder;

/// Config for Http service support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct HttpConfig {
    /// endpoint of this backend
    pub endpoint: Option<String>,
    /// username of this backend
    pub username: Option<String>,
    /// password of this backend
    pub password: Option<String>,
    /// token of this backend
    pub token: Option<String>,
    /// root of this backend
    pub root: Option<String>,
}

impl Debug for HttpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpConfig")
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for HttpConfig {
    type Builder = HttpBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert(
                "endpoint".to_string(),
                format!("{}://{}", uri.scheme(), authority),
            );
        }

        if let Some(root) = uri.root() {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        HttpBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::Operator;
    use opendal_core::OperatorUri;

    fn register_http() {
        let once = std::sync::Once::new();
        once.call_once(|| {
            let registry = opendal_core::OperatorRegistry::get();
            crate::register_http_service(registry);
        });
    }

    #[test]
    fn from_uri_sets_endpoint_and_root() {
        let uri = OperatorUri::new(
            "http://example.com/static/assets",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = HttpConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("http://example.com"));
        assert_eq!(cfg.root.as_deref(), Some("static/assets"));
    }

    #[test]
    fn from_uri_allows_missing_authority() {
        let uri = OperatorUri::new("http", Vec::<(String, String)>::new()).unwrap();

        let cfg = HttpConfig::from_uri(&uri).unwrap();
        assert!(cfg.endpoint.is_none());
    }

    #[test]
    fn from_uri_preserves_query_options() {
        let uri = OperatorUri::new(
            "http://cdn.example.com/data?token=abc123",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = HttpConfig::from_uri(&uri).unwrap();

        assert_eq!(cfg.endpoint.as_deref(), Some("http://cdn.example.com"));
        assert_eq!(cfg.token.as_deref(), Some("abc123"));
    }

    #[test]
    fn from_uri_ignores_endpoint_override() {
        let uri = OperatorUri::new(
            "http://example.com/data",
            vec![(
                "endpoint".to_string(),
                "https://cdn.example.com".to_string(),
            )],
        )
        .unwrap();
        let cfg = HttpConfig::from_uri(&uri).unwrap();

        assert_eq!(cfg.endpoint.as_deref(), Some("http://example.com"));
    }

    #[test]
    fn operator_from_uri_http() {
        register_http();
        let op = Operator::from_uri("http://example.com").unwrap();
        assert_eq!(op.info().scheme(), "http");
    }

    #[test]
    fn operator_from_uri_https() {
        register_http();
        let op = Operator::from_uri("https://example.com").unwrap();
        assert_eq!(op.info().scheme(), "http");
    }

    #[test]
    fn from_uri_with_https_scheme() {
        // "https" is an alias for "http" to support standard https:// URIs
        let uri = OperatorUri::new("https://example.com", Vec::<(String, String)>::new()).unwrap();
        let cfg = HttpConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("https://example.com"));
    }
}
