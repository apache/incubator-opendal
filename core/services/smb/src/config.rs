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

use std::fmt;

use opendal_core::Configurator;
use serde::Deserialize;
use serde::Serialize;

use super::backend::SmbBuilder;

/// Configuration for accessing an SMB share.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct SmbConfig {
    /// Required server hostname or IP address, with an optional port. The default port is 445.
    ///
    /// @example server.example.com:445
    pub endpoint: String,
    /// Required name of the SMB share.
    ///
    /// @example documents
    pub share: String,
    /// Root directory within the share. Defaults to `/`.
    pub root: Option<String>,
    /// NTLM username, optionally qualified as `DOMAIN\user` or `user@domain`.
    pub user: Option<String>,
    /// Password for the configured user.
    pub password: Option<String>,
}

impl fmt::Debug for SmbConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmbConfig")
            .field("endpoint", &self.endpoint)
            .field("share", &self.share)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Configurator for SmbConfig {
    type Builder = SmbBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), authority.to_string());
        }
        if let Some(user) = uri.username() {
            map.entry("user".to_string())
                .or_insert_with(|| user.to_string());
        }
        if let Some(password) = uri.password() {
            map.entry("password".to_string())
                .or_insert_with(|| password.to_string());
        }
        if let Some(path) = uri.root() {
            let (share, root) = path.split_once('/').unwrap_or((path, ""));
            map.insert("share".to_string(), share.to_string());
            if !root.is_empty() {
                map.insert("root".to_string(), root.to_string());
            }
        }
        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        SmbBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use opendal_core::OperatorUri;

    use super::*;

    #[test]
    fn uri_extracts_server_share_and_root() {
        let uri = OperatorUri::new(
            "smb://localhost:1445/data/documents",
            [("user".to_string(), "alice".to_string())],
        )
        .unwrap();
        let config = SmbConfig::from_uri(&uri).unwrap();
        assert_eq!(config.endpoint, "localhost:1445");
        assert_eq!(config.share, "data");
        assert_eq!(config.root.as_deref(), Some("documents"));
        assert_eq!(config.user.as_deref(), Some("alice"));
    }

    #[test]
    fn uri_preserves_explicit_root_for_share_only_uri() {
        let uri = OperatorUri::new(
            "smb://localhost/data",
            [("root".to_string(), "documents".to_string())],
        )
        .unwrap();
        assert_eq!(
            SmbConfig::from_uri(&uri).unwrap().root.as_deref(),
            Some("documents")
        );
    }

    #[test]
    fn debug_redacts_credentials() {
        let config = SmbConfig {
            user: Some("private-user".to_string()),
            password: Some("private-password".to_string()),
            ..Default::default()
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("private-user"));
        assert!(!debug.contains("private-password"));
    }

    #[test]
    fn uri_accepts_embedded_credentials_and_options_override_them() {
        let uri = OperatorUri::new(
            "smb://alice:uri-password@localhost/data",
            [("password".to_string(), "option-password".to_string())],
        )
        .unwrap();
        let config = SmbConfig::from_uri(&uri).unwrap();
        assert_eq!(config.user.as_deref(), Some("alice"));
        assert_eq!(config.password.as_deref(), Some("option-password"));
        assert!(!format!("{config:?}").contains(uri.password().unwrap()));
        assert!(!format!("{config:?}").contains("option-password"));
    }

    #[test]
    fn uri_embedded_credentials_are_used_without_options() {
        let uri = OperatorUri::new("smb://alice:uri-password@localhost/data", []).unwrap();
        let config = SmbConfig::from_uri(&uri).unwrap();
        assert_eq!(config.user.as_deref(), Some("alice"));
        assert_eq!(config.password.as_deref(), uri.password());
    }

    #[test]
    fn uri_rejects_missing_share() {
        let uri = OperatorUri::new("smb://localhost/", []).unwrap();
        let config = SmbConfig::from_uri(&uri).unwrap();
        let error = opendal_core::Operator::new(config.into_builder()).unwrap_err();
        assert_eq!(error.kind(), opendal_core::ErrorKind::ConfigInvalid);
    }
}
