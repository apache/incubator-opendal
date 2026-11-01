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

use super::backend::SftpBuilder;

/// Config for Sftp Service support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct SftpConfig {
    /// endpoint of this backend
    pub endpoint: Option<String>,
    /// root of this backend
    pub root: Option<String>,
    /// user of this backend
    pub user: Option<String>,
    /// key of this backend
    pub key: Option<String>,
    /// known_hosts_strategy of this backend
    pub known_hosts_strategy: Option<String>,
    /// Deprecated: SFTP copy capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "SFTP copy capability is enabled by default and this option is no longer needed."
    )]
    pub enable_copy: bool,
}

impl Debug for SftpConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SftpConfig")
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for SftpConfig {
    type Builder = SftpBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), authority.to_string());
        }

        if let Some(root) = uri.root() {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        SftpBuilder { config: self }
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
            "sftp://sftp.example.com/home/alice",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = SftpConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("sftp.example.com"));
        assert_eq!(cfg.root.as_deref(), Some("home/alice"));
    }

    #[test]
    fn from_uri_applies_connection_overrides() {
        let uri = OperatorUri::new(
            "sftp://host",
            vec![
                ("user".to_string(), "alice".to_string()),
                ("key".to_string(), "/home/alice/.ssh/id_rsa".to_string()),
                ("known_hosts_strategy".to_string(), "accept".to_string()),
            ],
        )
        .unwrap();

        let cfg = SftpConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("host"));
        assert_eq!(cfg.user.as_deref(), Some("alice"));
        assert_eq!(cfg.key.as_deref(), Some("/home/alice/.ssh/id_rsa"));
        assert_eq!(cfg.known_hosts_strategy.as_deref(), Some("accept"));
    }
}
