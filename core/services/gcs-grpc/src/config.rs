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

use serde::{Deserialize, Serialize};

use super::backend::GcsGrpcBuilder;

/// Configuration for the Google Cloud Storage gRPC service.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GcsGrpcConfig {
    /// Root path for all operations.
    pub root: Option<String>,
    /// Bucket name.
    #[serde(
        alias = "google_bucket",
        alias = "google_bucket_name",
        alias = "bucket_name"
    )]
    pub bucket: String,
    /// gRPC endpoint.
    pub endpoint: Option<String>,
    /// OAuth 2.0 scope.
    pub scope: Option<String>,
    /// Service account used by the GCE metadata server.
    #[serde(
        alias = "google_service_account",
        alias = "google_service_account_path",
        alias = "service_account_path"
    )]
    pub service_account: Option<String>,
    /// Base64-encoded service account credential.
    #[serde(alias = "google_service_account_key", alias = "service_account_key")]
    pub credential: Option<String>,
    /// Path to a service account credential file.
    #[serde(alias = "google_application_credentials")]
    pub credential_path: Option<String>,
    /// Send requests without authentication.
    #[serde(alias = "google_skip_signature")]
    pub skip_signature: bool,
    /// Disable the GCE metadata credential provider.
    pub disable_vm_metadata: bool,
    /// Disable environment and well-known credential loading.
    pub disable_config_load: bool,
    /// OAuth 2.0 access token.
    pub token: Option<String>,
}

impl Debug for GcsGrpcConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsGrpcConfig")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for GcsGrpcConfig {
    type Builder = GcsGrpcBuilder;

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
        GcsGrpcBuilder {
            config: self,
            credential_provider_chain: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use opendal_core::{Configurator, OperatorUri};

    use super::*;

    #[test]
    fn from_uri_extracts_bucket_and_root() {
        let uri = OperatorUri::new(
            "gcs-grpc://example-bucket/path/to/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = GcsGrpcConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket, "example-bucket");
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
    }
}
