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

use super::backend::B2Builder;

/// Config for backblaze b2 services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct B2Config {
    /// root of this backend.
    ///
    /// All operations will happen under this root.
    pub root: Option<String>,
    /// keyID of this backend.
    ///
    /// - If application_key_id is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub application_key_id: Option<String>,
    /// applicationKey of this backend.
    ///
    /// - If application_key is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub application_key: Option<String>,
    /// bucket of this backend.
    ///
    /// required.
    pub bucket: String,
    /// bucket id of this backend.
    ///
    /// required.
    pub bucket_id: String,
}

impl Debug for B2Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("B2Config")
            .field("root", &self.root)
            .field("application_key_id", &self.application_key_id)
            .field("bucket_id", &self.bucket_id)
            .field("bucket", &self.bucket)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for B2Config {
    type Builder = B2Builder;

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
        B2Builder { config: self }
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
            "b2://example-bucket/path/to/root",
            vec![("bucket_id".to_string(), "bucket-id".to_string())],
        )
        .unwrap();

        let cfg = B2Config::from_uri(&uri).unwrap();
        assert_eq!(cfg.bucket, "example-bucket");
        assert_eq!(cfg.root.as_deref(), Some("path/to/root"));
        assert_eq!(cfg.bucket_id, "bucket-id");
    }
}
