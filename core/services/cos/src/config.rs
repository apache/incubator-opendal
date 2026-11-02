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

use opendal_core::Configurator;
use opendal_core::OperatorUri;
use opendal_core::Result;

use super::backend::CosBuilder;

/// Tencent-Cloud COS services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
pub struct CosConfig {
    /// Root of this backend.
    pub root: Option<String>,
    /// Endpoint of this backend.
    pub endpoint: Option<String>,
    /// Secret ID of this backend.
    pub secret_id: Option<String>,
    /// Secret key of this backend.
    pub secret_key: Option<String>,
    /// Security token (a.k.a. session token) of this backend.
    ///
    /// This is used for temporary credentials issued by Tencent Cloud STS
    /// (e.g. `GetFederationToken` / `AssumeRole`). When `security_token` is
    /// provided, it will be used together with `secret_id` and `secret_key`
    /// to sign requests, and the `x-cos-security-token` header will be
    /// attached automatically by the signer.
    ///
    /// If this field is not set, OpenDAL will also fall back to reading
    /// the token from environment variables `TENCENTCLOUD_TOKEN`,
    /// `TENCENTCLOUD_SECURITY_TOKEN` or `QCLOUD_SECRET_TOKEN` (unless
    /// `disable_config_load` is enabled).
    pub security_token: Option<String>,
    /// Bucket of this backend.
    pub bucket: Option<String>,
    /// Deprecated: COS versioning capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "COS versioning capability is enabled by default and this option is no longer needed."
    )]
    pub enable_versioning: bool,
    /// Disable config load so that opendal will not load config from
    pub disable_config_load: bool,
}

impl Debug for CosConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CosConfig")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field(
                "security_token",
                &self.security_token.as_ref().map(|_| "<redacted>"),
            )
            .field("disable_config_load", &self.disable_config_load)
            .finish_non_exhaustive()
    }
}

impl Configurator for CosConfig {
    type Builder = CosBuilder;

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
        CosBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_extracts_bucket_and_root() {
        let uri = OperatorUri::new(
            "cos://example-bucket/path/to/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = CosConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket.as_deref(), Some("example-bucket"));
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
    }

    #[test]
    fn from_uri_accepts_cosn_scheme() {
        let uri = OperatorUri::new(
            "cosn://example-bucket/path/to/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = CosConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket.as_deref(), Some("example-bucket"));
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
    }

    #[test]
    fn from_uri_extracts_security_token() {
        let uri = OperatorUri::new(
            "cos://example-bucket/",
            vec![
                ("secret_id".to_string(), "id".to_string()),
                ("secret_key".to_string(), "key".to_string()),
                ("security_token".to_string(), "token".to_string()),
            ],
        )
        .unwrap();
        let cfg = CosConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.secret_id.as_deref(), Some("id"));
        assert_eq!(cfg.secret_key.as_deref(), Some("key"));
        assert_eq!(cfg.security_token.as_deref(), Some("token"));
    }

    #[test]
    fn debug_redacts_security_token() {
        let cfg = CosConfig {
            security_token: Some("super-secret-token".to_string()),
            ..Default::default()
        };
        let debug_output = format!("{cfg:?}");
        assert!(!debug_output.contains("super-secret-token"));
        assert!(debug_output.contains("<redacted>"));
    }
}
