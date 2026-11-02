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

use opendal_core::{Configurator, OperatorUri, Result};
use serde::Deserialize;
use serde::Serialize;

use super::backend::OnedriveBuilder;

/// Config for [OneDrive](https://onedrive.com) backend support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct OnedriveConfig {
    /// The root path for the OneDrive service for the file access
    pub root: Option<String>,
    /// Microsoft Graph API (also OneDrive API) access token
    pub access_token: Option<String>,
    ///  Microsoft Graph API (also OneDrive API) refresh token
    pub refresh_token: Option<String>,
    /// Microsoft Graph API Application (client) ID that is in the Azure's app registration portal
    pub client_id: Option<String>,
    /// Microsoft Graph API Application client secret that is in the Azure's app registration portal
    pub client_secret: Option<String>,
    /// Deprecated: OneDrive supports version listing without this option.
    #[deprecated(
        since = "0.57.0",
        note = "OneDrive supports version listing without this option."
    )]
    pub enable_versioning: bool,
}

impl Debug for OnedriveConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnedriveConfig")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Configurator for OnedriveConfig {
    type Builder = OnedriveBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
        let mut map = uri.options().clone();

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            let normalized = match root.split_once('/') {
                Some((_, rest)) if !rest.is_empty() => rest.to_string(),
                _ => root.to_string(),
            };
            if !normalized.is_empty() {
                map.insert("root".to_string(), normalized);
            }
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        OnedriveBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_root() {
        let uri = OperatorUri::new(
            "onedrive://drive/root/documents",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = OnedriveConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.root.as_deref(), Some("documents"));
    }
}
