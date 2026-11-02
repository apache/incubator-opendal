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

use super::backend::AzblobBuilder;

/// Azure Storage Blob services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AzblobConfig {
    /// The root of Azblob service backend.
    ///
    /// All operations will happen under this root.
    pub root: Option<String>,

    /// The container name of Azblob service backend.
    #[serde(alias = "azure_container_name", alias = "container_name")]
    pub container: String,

    /// The endpoint of Azblob service backend.
    ///
    /// Endpoint must be full uri, e.g.
    ///
    /// - Azblob: `https://accountname.blob.core.windows.net`
    /// - Azurite: `http://127.0.0.1:10000/devstoreaccount1`
    #[serde(alias = "azure_storage_endpoint", alias = "azure_endpoint")]
    pub endpoint: Option<String>,

    /// The account name of Azblob service backend.
    #[serde(alias = "azure_storage_account_name")]
    pub account_name: Option<String>,

    /// The account key of Azblob service backend.
    #[serde(
        alias = "azure_storage_account_key",
        alias = "azure_storage_access_key",
        alias = "azure_storage_master_key",
        alias = "access_key",
        alias = "master_key"
    )]
    pub account_key: Option<String>,

    /// The Base64-encoded AES-256 customer-provided encryption key.
    ///
    /// The key must decode to exactly 32 bytes. Building an operator returns
    /// `ConfigInvalid` for invalid Base64 or a different decoded length.
    pub encryption_key: Option<String>,

    /// The encryption key sha256 of Azblob service backend.
    pub encryption_key_sha256: Option<String>,

    /// The encryption algorithm of Azblob service backend.
    pub encryption_algorithm: Option<String>,

    /// The sas token of Azblob service backend.
    #[serde(
        alias = "azure_storage_sas_key",
        alias = "azure_storage_sas_token",
        alias = "sas_key"
    )]
    pub sas_token: Option<String>,

    /// Deprecated: Azblob delete batch capability is enabled by default with Azure Blob's 256-operation batch limit.
    #[deprecated(
        since = "0.57.0",
        note = "Azblob delete batch capability is enabled by default with Azure Blob's 256-operation batch limit. Use CapabilityOverrideLayer to override delete_max_size for specific endpoints."
    )]
    pub batch_max_operations: Option<usize>,

    /// Skip signature will skip loading credentials and signing requests.
    #[serde(default)]
    pub skip_signature: bool,
}

impl Debug for AzblobConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzblobConfig")
            .field("root", &self.root)
            .field("container", &self.container)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for AzblobConfig {
    type Builder = AzblobBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(container) = uri.name() {
            map.insert("container".to_string(), container.to_string());
        }

        if let Some(root) = uri.root() {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    #[allow(deprecated)]
    fn into_builder(self) -> Self::Builder {
        AzblobBuilder {
            config: self,
            credential_providers: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn test_container_name_aliases() {
        let json = r#"{"container": "test-container"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.container, "test-container");

        let json = r#"{"azure_container_name": "test-container"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.container, "test-container");

        let json = r#"{"container_name": "test-container"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.container, "test-container");
    }

    #[test]
    fn test_account_name_aliases() {
        let json = r#"{"container": "test", "account_name": "testaccount"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.account_name, Some("testaccount".to_string()));

        let json = r#"{"container": "test", "azure_storage_account_name": "testaccount-azure"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.account_name, Some("testaccount-azure".to_string()));
    }

    #[test]
    fn test_account_key_aliases() {
        let json = r#"{"container": "test", "account_key": "dGVzdGtleQ=="}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.account_key, Some("dGVzdGtleQ==".to_string()));

        let json = r#"{"container": "test", "azure_storage_account_key": "dGVzdGtleQ=="}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.account_key, Some("dGVzdGtleQ==".to_string()));

        let json = r#"{"container": "test", "azure_storage_access_key": "dGVzdGtleQ=="}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.account_key, Some("dGVzdGtleQ==".to_string()));

        let json = r#"{"container": "test", "azure_storage_master_key": "dGVzdGtleQ=="}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.account_key, Some("dGVzdGtleQ==".to_string()));

        let json = r#"{"container": "test", "access_key": "dGVzdGtleQ=="}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.account_key, Some("dGVzdGtleQ==".to_string()));

        let json = r#"{"container": "test", "master_key": "dGVzdGtleQ=="}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.account_key, Some("dGVzdGtleQ==".to_string()));
    }

    #[test]
    fn test_sas_token_aliases() {
        let json = r#"{"container": "test", "sas_token": "test-token"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.sas_token, Some("test-token".to_string()));

        let json = r#"{"container": "test", "azure_storage_sas_key": "test-token"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.sas_token, Some("test-token".to_string()));

        let json = r#"{"container": "test", "azure_storage_sas_token": "test-token"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.sas_token, Some("test-token".to_string()));

        let json = r#"{"container": "test", "sas_key": "test-token"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.sas_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_endpoint_aliases() {
        let json = r#"{"container": "test", "endpoint": "https://test.blob.core.windows.net"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.endpoint,
            Some("https://test.blob.core.windows.net".to_string())
        );

        let json = r#"{"container": "test", "azure_storage_endpoint": "https://test.blob.core.windows.net"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.endpoint,
            Some("https://test.blob.core.windows.net".to_string())
        );

        let json =
            r#"{"container": "test", "azure_endpoint": "https://test.blob.core.windows.net"}"#;
        let config: AzblobConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.endpoint,
            Some("https://test.blob.core.windows.net".to_string())
        );
    }

    #[test]
    fn from_uri_with_host_container() {
        let uri = OperatorUri::new(
            "azblob://my-container/path/to/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = AzblobConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.container, "my-container");
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
    }

    #[test]
    fn from_uri_with_path_container() {
        let uri = OperatorUri::new(
            "azblob://my-container/nested/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = AzblobConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.container, "my-container");
        assert_eq!(cfg.root.as_deref(), Some("nested/root"));
    }
}
