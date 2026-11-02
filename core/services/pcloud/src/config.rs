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

use opendal_core::*;
use serde::Deserialize;
use serde::Serialize;

use super::backend::PcloudBuilder;

/// Config for Pcloud services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct PcloudConfig {
    /// root of this backend.
    ///
    /// All operations will happen under this root.
    pub root: Option<String>,
    /// pCloud endpoint address.
    pub endpoint: String,
    /// pCloud username.
    pub username: Option<String>,
    /// pCloud password.
    pub password: Option<String>,
}

impl Debug for PcloudConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcloudConfig")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

impl Configurator for PcloudConfig {
    type Builder = PcloudBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), format!("https://{authority}"));
        }

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        PcloudBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_uri_sets_endpoint_and_root() -> Result<()> {
        let uri = OperatorUri::new(
            "pcloud://api.pcloud.com/drive/photos",
            vec![("username".to_string(), "alice".to_string())],
        )?;

        let cfg = PcloudConfig::from_uri(&uri)?;
        assert_eq!(cfg.endpoint, "https://api.pcloud.com".to_string());
        assert_eq!(cfg.root.as_deref(), Some("drive/photos"));
        assert_eq!(cfg.username.as_deref(), Some("alice"));
        Ok(())
    }

    #[test]
    fn from_uri_allows_missing_authority() -> Result<()> {
        let uri = OperatorUri::new("pcloud", Vec::<(String, String)>::new())?;

        let cfg = PcloudConfig::from_uri(&uri)?;
        assert!(cfg.endpoint.is_empty());
        Ok(())
    }
}
