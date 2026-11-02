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

use super::backend::WebdavBuilder;

/// Config for [WebDAV](https://datatracker.ietf.org/doc/html/rfc4918) backend support.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct WebdavConfig {
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
    /// Deprecated: WebDAV copy capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "WebDAV copy capability is enabled by default and this option is no longer needed."
    )]
    pub disable_copy: bool,
    /// Disable automatic parent directory creation before write operations.
    ///
    /// By default, OpenDAL creates parent directories using MKCOL before writing files.
    /// This requires PROPFIND support to check directory existence.
    ///
    /// Some WebDAV-compatible servers (e.g., bazel-remote) don't support PROPFIND
    /// or don't require explicit directory creation. Enable this option to skip
    /// the MKCOL calls and write files directly.
    ///
    /// Default: false
    pub disable_create_dir: bool,
    /// Deprecated: WebDAV user metadata capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "WebDAV user metadata capability is enabled by default. Use CapabilityOverrideLayer to override write_with_user_metadata for endpoints without PROPPATCH support."
    )]
    pub enable_user_metadata: bool,
    /// The XML namespace prefix for user metadata properties.
    ///
    /// This prefix is used in PROPPATCH/PROPFIND XML requests.
    /// Different servers may require different prefixes.
    ///
    /// Default: "opendal"
    pub user_metadata_prefix: Option<String>,
    /// The XML namespace URI for user metadata properties.
    ///
    /// This URI uniquely identifies the namespace for custom properties.
    /// Different servers may require different namespace URIs.
    /// For example, Nextcloud might work better with its own namespace.
    ///
    /// Default: `https://opendal.apache.org/ns`
    pub user_metadata_uri: Option<String>,
    /// Deprecated: WebDAV conditional read capabilities are enabled by default.
    #[deprecated(
        since = "0.60.0",
        note = "WebDAV conditional read capabilities are enabled by default. Use CapabilityOverrideLayer to override read_with_if_match, read_with_if_none_match, read_with_if_modified_since and read_with_if_unmodified_since for endpoints without ETag support."
    )]
    pub enable_conditional_read: bool,
}

#[allow(deprecated)]
impl Default for WebdavConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            username: None,
            password: None,
            token: None,
            root: None,
            disable_copy: false,
            disable_create_dir: false,
            enable_user_metadata: false,
            user_metadata_prefix: None,
            user_metadata_uri: None,
            enable_conditional_read: true,
        }
    }
}

impl Debug for WebdavConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebdavConfig")
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .field("root", &self.root)
            .field("disable_create_dir", &self.disable_create_dir)
            .field("user_metadata_prefix", &self.user_metadata_prefix)
            .field("user_metadata_uri", &self.user_metadata_uri)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for WebdavConfig {
    type Builder = WebdavBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), format!("https://{authority}"));
        }

        if let Some(root) = uri.root() {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        WebdavBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_endpoint_and_root() {
        let uri = OperatorUri::new(
            "webdav://webdav.example.com/remote.php/webdav",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = WebdavConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("https://webdav.example.com"));
        assert_eq!(cfg.root.as_deref(), Some("remote.php/webdav"));
    }

    #[test]
    fn from_uri_ignores_endpoint_override() {
        let uri = OperatorUri::new(
            "webdav://dav.internal/data",
            vec![(
                "endpoint".to_string(),
                "http://dav.internal:8080".to_string(),
            )],
        )
        .unwrap();

        let cfg = WebdavConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("https://dav.internal"));
    }

    #[test]
    #[allow(deprecated)]
    fn from_uri_accepts_deprecated_disable_copy() {
        let uri = OperatorUri::new(
            "webdav://dav.example.com",
            vec![("disable_copy".to_string(), "true".to_string())],
        )
        .unwrap();

        let cfg = WebdavConfig::from_uri(&uri).unwrap();
        assert!(cfg.disable_copy);
    }

    #[test]
    fn from_uri_propagates_disable_create_dir() {
        let uri = OperatorUri::new(
            "webdav://dav.example.com",
            vec![("disable_create_dir".to_string(), "true".to_string())],
        )
        .unwrap();

        let cfg = WebdavConfig::from_uri(&uri).unwrap();
        assert!(cfg.disable_create_dir);
    }
}
