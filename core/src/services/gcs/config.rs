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
use std::fmt::Formatter;

use serde::Deserialize;
use serde::Serialize;

/// [Google Cloud Storage](https://cloud.google.com/storage) services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GcsConfig {
    /// root URI, all operations happens under `root`
    pub root: Option<String>,
    /// bucket name
    pub bucket: String,
    /// endpoint URI of GCS service,
    /// default is `https://storage.googleapis.com`
    pub endpoint: Option<String>,
    /// Scope for gcs.
    pub scope: Option<String>,
    /// Service Account for gcs.
    pub service_account: Option<String>,
    /// Credentials string for GCS service OAuth2 authentication.
    pub credential: Option<String>,
    /// Local path to credentials file for GCS service OAuth2 authentication.
    pub credential_path: Option<String>,
    /// The predefined acl for GCS.
    pub predefined_acl: Option<String>,
    /// The default storage class used by gcs.
    pub default_storage_class: Option<String>,
    /// Allow opendal to send requests without signing when credentials are not
    /// loaded.
    pub allow_anonymous: bool,
    /// Disable attempting to load credentials from the GCE metadata server when
    /// running within Google Cloud.
    pub disable_vm_metadata: bool,
    /// Disable loading configuration from the environment.
    pub disable_config_load: bool,
    /// Enable versioning for the bucket.
    pub enable_versioning: bool,
    /// A Google Cloud OAuth2 token.
    ///
    /// Takes precedence over `credential` and `credential_path`.
    pub token: Option<String>,
}

impl Debug for GcsConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcsConfig")
            .field("root", &self.root)
            .field("bucket", &self.bucket)
            .field("endpoint", &self.endpoint)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}
