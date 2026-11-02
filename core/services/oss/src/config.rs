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

use opendal_core::OperatorUri;

use super::backend::OssBuilder;

/// Config for Aliyun Object Storage Service (OSS) support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct OssConfig {
    /// Root for oss.
    pub root: Option<String>,

    /// Endpoint for oss.
    pub endpoint: Option<String>,
    /// Presign endpoint for oss.
    pub presign_endpoint: Option<String>,
    /// Bucket for oss.
    pub bucket: String,
    /// Addressing style for oss.
    pub addressing_style: Option<String>,
    /// Pre sign addressing style for oss.
    pub presign_addressing_style: Option<String>,

    /// Deprecated: OSS versioning capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "OSS versioning capability is enabled by default and this option is no longer needed."
    )]
    pub enable_versioning: bool,

    // OSS features
    /// Server side encryption for oss.
    pub server_side_encryption: Option<String>,
    /// Server side encryption key id for oss.
    pub server_side_encryption_key_id: Option<String>,
    /// Skip signature will skip loading credentials and signing requests.
    pub skip_signature: bool,
    /// Allow anonymous for oss.
    #[deprecated(
        since = "0.57.0",
        note = "Please use `skip_signature` instead of `allow_anonymous`"
    )]
    pub allow_anonymous: bool,

    // authenticate options
    /// Access key id for oss.
    ///
    /// - this field if it's `is_some`
    /// - env value: `ALIBABA_CLOUD_ACCESS_KEY_ID`
    pub access_key_id: Option<String>,
    /// Access key secret for oss.
    ///
    /// - this field if it's `is_some`
    /// - env value: `ALIBABA_CLOUD_ACCESS_KEY_SECRET`
    pub access_key_secret: Option<String>,
    /// `security_token` will be loaded from
    ///
    /// - this field if it's `is_some`
    /// - env value: `ALIBABA_CLOUD_SECURITY_TOKEN`
    pub security_token: Option<String>,
    /// Deprecated: OSS delete batch capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "OSS delete batch capability is enabled by default. Use CapabilityOverrideLayer to override delete_max_size for specific endpoints."
    )]
    pub batch_max_operations: Option<usize>,
    /// Deprecated: OSS delete batch capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "OSS delete batch capability is enabled by default. Use CapabilityOverrideLayer to override delete_max_size for specific endpoints."
    )]
    pub delete_max_size: Option<usize>,
    /// If `role_arn` is set, we will use already known config as source
    /// credential to assume role with `role_arn`.
    ///
    /// - this field if it's `is_some`
    /// - env value: `ALIBABA_CLOUD_ROLE_ARN`
    pub role_arn: Option<String>,
    /// role_session_name for this backend.
    pub role_session_name: Option<String>,
    /// `oidc_provider_arn` will be loaded from
    ///
    /// - this field if it's `is_some`
    /// - env value: `ALIBABA_CLOUD_OIDC_PROVIDER_ARN`
    pub oidc_provider_arn: Option<String>,
    /// `oidc_token_file` will be loaded from
    ///
    /// - this field if it's `is_some`
    /// - env value: `ALIBABA_CLOUD_OIDC_TOKEN_FILE`
    pub oidc_token_file: Option<String>,
    /// `sts_endpoint` will be loaded from
    ///
    /// - this field if it's `is_some`
    /// - env value: `ALIBABA_CLOUD_STS_ENDPOINT`
    pub sts_endpoint: Option<String>,
    /// external_id for this backend.
    pub external_id: Option<String>,
}

impl Debug for OssConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Builder")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("skip_signature", &self.skip_signature)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for OssConfig {
    type Builder = OssBuilder;

    fn from_uri(uri: &OperatorUri) -> opendal_core::Result<Self> {
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
        OssBuilder { config: self }
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
            "oss://example-bucket/path/to/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = OssConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket, "example-bucket");
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
    }
}
