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

use super::backend::VercelArtifactsBuilder;

/// Config for Vercel Cache support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct VercelArtifactsConfig {
    /// The access token for Vercel.
    pub access_token: Option<String>,
    /// The endpoint for the Vercel artifacts API.
    ///
    /// Defaults to `https://api.vercel.com`.
    pub endpoint: Option<String>,
    /// The Vercel team ID. When set, the `teamId` query parameter
    /// is appended to all API requests.
    pub team_id: Option<String>,
    /// The Vercel team slug. When set, the `slug` query parameter
    /// is appended to all API requests.
    pub team_slug: Option<String>,
}

impl Debug for VercelArtifactsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VercelArtifactsConfig")
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for VercelArtifactsConfig {
    type Builder = VercelArtifactsBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        Self::from_iter(uri.options().clone())
    }

    fn into_builder(self) -> Self::Builder {
        VercelArtifactsBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_loads_access_token() {
        let uri = OperatorUri::new(
            "vercel-artifacts://cache",
            vec![("access_token".to_string(), "token123".to_string())],
        )
        .unwrap();

        let cfg = VercelArtifactsConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.access_token.as_deref(), Some("token123"));
    }

    #[test]
    fn from_uri_loads_all_options() {
        let uri = OperatorUri::new(
            "vercel-artifacts://cache",
            vec![
                ("access_token".to_string(), "token123".to_string()),
                (
                    "endpoint".to_string(),
                    "https://custom.api.example.com".to_string(),
                ),
                ("team_id".to_string(), "team_abc".to_string()),
                ("team_slug".to_string(), "my-team".to_string()),
            ],
        )
        .unwrap();

        let cfg = VercelArtifactsConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.access_token.as_deref(), Some("token123"));
        assert_eq!(
            cfg.endpoint.as_deref(),
            Some("https://custom.api.example.com")
        );
        assert_eq!(cfg.team_id.as_deref(), Some("team_abc"));
        assert_eq!(cfg.team_slug.as_deref(), Some("my-team"));
    }

    #[test]
    fn defaults_are_none() {
        let cfg = VercelArtifactsConfig::default();
        assert!(cfg.access_token.is_none());
        assert!(cfg.endpoint.is_none());
        assert!(cfg.team_id.is_none());
        assert!(cfg.team_slug.is_none());
    }
}
