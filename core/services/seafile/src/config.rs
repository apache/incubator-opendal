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

use super::SEAFILE_SCHEME;
use super::backend::SeafileBuilder;

/// Config for seafile services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct SeafileConfig {
    /// root of this backend.
    ///
    /// All operations will happen under this root.
    pub root: Option<String>,
    /// endpoint address of this backend.
    pub endpoint: Option<String>,
    /// username of this backend.
    pub username: Option<String>,
    /// password of this backend.
    pub password: Option<String>,
    /// repo_name of this backend.
    ///
    /// required.
    pub repo_name: String,
}

impl Debug for SeafileConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeafileConfig")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("username", &self.username)
            .field("repo_name", &self.repo_name)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for SeafileConfig {
    type Builder = SeafileBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), format!("https://{authority}"));
        }
        if let Some(raw_path) = uri.root() {
            let mut segments = raw_path.splitn(2, '/');
            if let Some(repo_name) = segments.next().filter(|s| !s.is_empty()) {
                map.insert("repo_name".to_string(), repo_name.to_string());
            }

            if let Some(rest) = segments.next()
                && !rest.is_empty()
            {
                map.insert("root".to_string(), rest.to_string());
            }
        }

        if !map.contains_key("repo_name") {
            return Err(opendal_core::Error::new(
                opendal_core::ErrorKind::ConfigInvalid,
                "repo_name is required via uri path or option",
            )
            .with_context("service", SEAFILE_SCHEME));
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        SeafileBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_endpoint_repo_and_root() {
        let uri = OperatorUri::new(
            "seafile://files.example.com/myrepo/projects/app",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = SeafileConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint.as_deref(), Some("https://files.example.com"));
        assert_eq!(cfg.repo_name, "myrepo".to_string());
        assert_eq!(cfg.root.as_deref(), Some("projects/app"));
    }
}
