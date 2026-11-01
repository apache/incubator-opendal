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

use super::backend::ObsBuilder;

/// Config for Huawei-Cloud Object Storage Service (OBS) support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct ObsConfig {
    /// Root for obs.
    pub root: Option<String>,
    /// Endpoint for obs.
    pub endpoint: Option<String>,
    /// Access key id for obs.
    pub access_key_id: Option<String>,
    /// Secret access key for obs.
    pub secret_access_key: Option<String>,
    /// Bucket for obs.
    pub bucket: Option<String>,
    /// Deprecated: OBS versioning capability is not controlled by service config.
    #[deprecated(
        since = "0.57.0",
        note = "OBS versioning capability is not controlled by this option and this option is no longer needed."
    )]
    pub enable_versioning: bool,
}

impl Debug for ObsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObsConfig")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for ObsConfig {
    type Builder = ObsBuilder;

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
        ObsBuilder { config: self }
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
            "obs://example-bucket/path/to/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();
        let cfg = ObsConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket.as_deref(), Some("example-bucket"));
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
    }
}
