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
use serde::Deserialize;
use serde::Serialize;

use crate::backend::TosBuilder;

/// Config for Volcengine TOS service.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct TosConfig {
    /// root of this backend.
    ///
    /// All operations will happen under this root.
    ///
    /// default to `/` if not set.
    pub root: Option<String>,
    /// bucket name of this backend.
    ///
    /// required.
    pub bucket: String,
    /// endpoint of this backend.
    ///
    /// Endpoint must be full uri, e.g.
    /// - TOS: `https://tos-cn-beijing.volces.com`
    /// - TOS with region: `https://tos-{region}.volces.com`
    ///
    /// If user inputs endpoint without scheme like "tos-cn-beijing.volces.com", we
    /// will prepend "https://" before it.
    pub endpoint: Option<String>,
    /// Region represent the signing region of this endpoint.
    ///
    /// Required if endpoint is not provided.
    ///
    /// - If region is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    /// - If still not set, default to `cn-beijing`.
    pub region: Option<String>,
    /// access_key_id of this backend.
    ///
    /// - If access_key_id is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    #[serde(alias = "tos_access_key_id", alias = "volcengine_access_key_id")]
    pub access_key_id: Option<String>,
    /// secret_access_key of this backend.
    ///
    /// - If secret_access_key is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    #[serde(
        alias = "tos_secret_access_key",
        alias = "volcengine_secret_access_key"
    )]
    pub secret_access_key: Option<String>,
    /// security_token of this backend.
    ///
    /// This token will expire after sometime, it's recommended to set security_token
    /// by hand.
    #[serde(alias = "tos_security_token", alias = "volcengine_session_token")]
    pub security_token: Option<String>,
    /// Disable config load so that opendal will not load config from
    /// environment.
    ///
    /// For examples:
    /// - envs like `TOS_ACCESS_KEY_ID`
    pub disable_config_load: bool,
    /// Skip signature will skip loading credentials and signing requests.
    pub skip_signature: bool,
}

impl Debug for TosConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TosConfig")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .finish_non_exhaustive()
    }
}

impl Configurator for TosConfig {
    type Builder = TosBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
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
        TosBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn test_tos_config_original_field_names() {
        let json = r#"{
            "bucket": "test-bucket",
            "access_key_id": "test-key",
            "secret_access_key": "test-secret",
            "region": "cn-beijing",
            "endpoint": "https://tos-cn-beijing.volces.com"
        }"#;

        let config: TosConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.bucket, "test-bucket");
        assert_eq!(config.access_key_id, Some("test-key".to_string()));
        assert_eq!(config.secret_access_key, Some("test-secret".to_string()));
        assert_eq!(config.region, Some("cn-beijing".to_string()));
        assert_eq!(
            config.endpoint,
            Some("https://tos-cn-beijing.volces.com".to_string())
        );
    }

    #[test]
    fn test_tos_config_tos_prefixed_aliases() {
        let json = r#"{
            "tos_access_key_id": "test-key",
            "tos_secret_access_key": "test-secret",
            "tos_security_token": "test-token"
        }"#;

        let config: TosConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.access_key_id, Some("test-key".to_string()));
        assert_eq!(config.secret_access_key, Some("test-secret".to_string()));
        assert_eq!(config.security_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_tos_config_volcengine_prefixed_aliases() {
        let json = r#"{
            "volcengine_access_key_id": "test-key",
            "volcengine_secret_access_key": "test-secret",
            "volcengine_session_token": "test-token"
        }"#;

        let config: TosConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.access_key_id, Some("test-key".to_string()));
        assert_eq!(config.secret_access_key, Some("test-secret".to_string()));
        assert_eq!(config.security_token, Some("test-token".to_string()));
    }

    #[test]
    fn from_uri_extracts_bucket_and_root() {
        let uri = OperatorUri::new("tos://example-bucket/path/to/root", iter::empty()).unwrap();
        let cfg = TosConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket, "example-bucket");
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
    }

    #[test]
    fn from_uri_extracts_endpoint() {
        let uri = OperatorUri::new(
            "tos://example-bucket/path/to/root?endpoint=https%3A%2F%2Fcustom-tos-endpoint.com",
            iter::empty(),
        )
        .unwrap();
        let cfg = TosConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket, "example-bucket");
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
        assert_eq!(
            cfg.endpoint.as_deref(),
            Some("https://custom-tos-endpoint.com")
        );
    }
}
