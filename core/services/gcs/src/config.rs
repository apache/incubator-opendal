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

use super::backend::GcsBuilder;

/// [Google Cloud Storage](https://cloud.google.com/storage) services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GcsConfig {
    /// root URI, all operations happens under `root`
    ///
    /// <!-- @group General -->
    /// <!-- @default / -->
    pub root: Option<String>,
    /// bucket name
    ///
    /// <!-- @group General -->
    /// <!-- @example my-bucket -->
    #[serde(
        alias = "google_bucket",
        alias = "google_bucket_name",
        alias = "bucket_name"
    )]
    pub bucket: String,
    /// endpoint URI of GCS service,
    /// default is `https://storage.googleapis.com`
    ///
    /// <!-- @group General -->
    /// <!-- @default https://storage.googleapis.com -->
    pub endpoint: Option<String>,
    /// Scope for gcs.
    ///
    /// <!-- @group General -->
    pub scope: Option<String>,
    /// Service Account for gcs.
    ///
    /// <!-- @group Credentials -->
    #[serde(
        alias = "google_service_account",
        alias = "google_service_account_path",
        alias = "service_account_path"
    )]
    pub service_account: Option<String>,
    /// Credentials string for GCS service OAuth2 authentication.
    ///
    /// <!-- @group Credentials -->
    #[serde(alias = "google_service_account_key", alias = "service_account_key")]
    pub credential: Option<String>,
    /// Local path to credentials file for GCS service OAuth2 authentication.
    ///
    /// <!-- @group Credentials -->
    #[serde(alias = "google_application_credentials")]
    pub credential_path: Option<String>,
    /// The predefined acl for GCS.
    ///
    /// <!-- @group Behavior -->
    pub predefined_acl: Option<String>,
    /// The default storage class used by gcs.
    ///
    /// <!-- @group Behavior -->
    pub default_storage_class: Option<String>,
    /// Skip signature will skip loading credentials and signing requests.
    ///
    /// <!-- @group Credentials -->
    #[serde(alias = "google_skip_signature")]
    pub skip_signature: bool,
    /// Allow opendal to send requests without signing when credentials are not
    /// loaded.
    ///
    /// <!-- @group Deprecated -->
    #[deprecated(
        since = "0.57.0",
        note = "Please use `skip_signature` instead of `allow_anonymous`"
    )]
    pub allow_anonymous: bool,
    /// Disable attempting to load credentials from the GCE metadata server when
    /// running within Google Cloud.
    ///
    /// <!-- @group Credentials -->
    pub disable_vm_metadata: bool,
    /// Disable loading configuration from the environment.
    ///
    /// <!-- @group Credentials -->
    pub disable_config_load: bool,
    /// A Google Cloud OAuth2 token.
    ///
    /// Takes precedence over `credential` and `credential_path`.
    ///
    /// <!-- @group Credentials -->
    pub token: Option<String>,
}

impl Debug for GcsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsConfig")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for GcsConfig {
    type Builder = GcsBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(name) = uri.name() {
            map.insert("bucket".to_string(), name.to_string());
        }

        if let Some(root) = uri.root() {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        GcsBuilder {
            config: self,
            credential_provider_chain: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::{Configurator, OperatorUri};

    #[test]
    fn test_bucket_aliases() {
        let config_json = r#"{"google_bucket": "test-bucket"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!("test-bucket", config.bucket);

        let config_json = r#"{"google_bucket_name": "test-bucket-name"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!("test-bucket-name", config.bucket);

        let config_json = r#"{"bucket_name": "test-bucket-alias"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!("test-bucket-alias", config.bucket);
    }

    #[test]
    fn test_service_account_aliases() {
        let config_json = r#"{"google_service_account": "/path/to/sa.json"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!(Some("/path/to/sa.json".to_string()), config.service_account);

        let config_json = r#"{"google_service_account_path": "/path/to/sa2.json"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!(
            Some("/path/to/sa2.json".to_string()),
            config.service_account
        );

        let config_json = r#"{"service_account_path": "/path/to/sa3.json"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!(
            Some("/path/to/sa3.json".to_string()),
            config.service_account
        );
    }

    #[test]
    fn test_credential_aliases() {
        let config_json = r#"{"google_service_account_key": "key-content"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!(Some("key-content".to_string()), config.credential);

        let config_json = r#"{"service_account_key": "key-content-2"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!(Some("key-content-2".to_string()), config.credential);
    }

    #[test]
    fn test_credential_path_aliases() {
        let config_json = r#"{"google_application_credentials": "/path/to/app.json"}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert_eq!(
            Some("/path/to/app.json".to_string()),
            config.credential_path
        );
    }

    #[test]
    fn test_skip_signature_aliases() {
        let config_json = r#"{"google_skip_signature": true}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert!(config.skip_signature);

        let config_json = r#"{"skip_signature": true}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        assert!(config.skip_signature);

        let config_json = r#"{"allow_anonymous": true}"#;
        let config: GcsConfig = serde_json::from_str(config_json).unwrap();
        #[allow(deprecated)]
        {
            assert!(config.allow_anonymous);
        }
    }

    #[test]
    fn from_uri_extracts_bucket_and_root() {
        let uri = OperatorUri::new(
            "gcs://example-bucket/path/to/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = GcsConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket, "example-bucket");
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
    }
}
