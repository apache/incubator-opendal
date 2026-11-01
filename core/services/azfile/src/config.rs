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

use super::AZFILE_SCHEME;
use super::backend::AzfileBuilder;

/// Azure File services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct AzfileConfig {
    /// The root path for azfile.
    pub root: Option<String>,
    /// The endpoint for azfile.
    pub endpoint: Option<String>,
    /// The share name for azfile.
    pub share_name: String,
    /// The account name for azfile.
    pub account_name: Option<String>,
    /// The account key for azfile.
    pub account_key: Option<String>,
    /// The sas token for azfile.
    pub sas_token: Option<String>,
}

impl Debug for AzfileConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzfileConfig")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("share_name", &self.share_name)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for AzfileConfig {
    type Builder = AzfileBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), format!("https://{authority}"));
        }

        if let Some(account) = uri
            .name()
            .and_then(|host| host.split('.').next())
            .filter(|account| !account.is_empty())
        {
            map.entry("account_name".to_string())
                .or_insert_with(|| account.to_string());
        }

        if let Some(root) = uri.root() {
            if let Some((share, rest)) = root.split_once('/') {
                if share.is_empty() {
                    return Err(opendal_core::Error::new(
                        opendal_core::ErrorKind::ConfigInvalid,
                        "share name is required in uri path",
                    )
                    .with_context("service", AZFILE_SCHEME));
                }
                map.insert("share_name".to_string(), share.to_string());
                if !rest.is_empty() {
                    map.insert("root".to_string(), rest.to_string());
                }
            } else if !root.is_empty() {
                map.insert("share_name".to_string(), root.to_string());
            }
        }

        if !map.contains_key("share_name") {
            return Err(opendal_core::Error::new(
                opendal_core::ErrorKind::ConfigInvalid,
                "share name is required",
            )
            .with_context("service", AZFILE_SCHEME));
        }

        Self::from_iter(map)
    }

    #[allow(deprecated)]
    fn into_builder(self) -> Self::Builder {
        AzfileBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_endpoint_share_root_and_account() {
        let uri = OperatorUri::new(
            "azfile://account.file.core.windows.net/share/documents/reports",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = AzfileConfig::from_uri(&uri).unwrap();
        assert_eq!(
            cfg.endpoint.as_deref(),
            Some("https://account.file.core.windows.net")
        );
        assert_eq!(cfg.share_name, "share".to_string());
        assert_eq!(cfg.root.as_deref(), Some("documents/reports"));
        assert_eq!(cfg.account_name.as_deref(), Some("account"));
    }

    #[test]
    fn from_uri_accepts_share_from_query() {
        let uri = OperatorUri::new(
            "azfile://account.file.core.windows.net",
            vec![("share_name".to_string(), "data".to_string())],
        )
        .unwrap();

        let cfg = AzfileConfig::from_uri(&uri).unwrap();
        assert_eq!(
            cfg.endpoint.as_deref(),
            Some("https://account.file.core.windows.net")
        );
        assert_eq!(cfg.share_name, "data".to_string());
    }
}
