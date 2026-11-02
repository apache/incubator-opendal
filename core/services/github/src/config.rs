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

use super::GITHUB_SCHEME;
use super::backend::GithubBuilder;
use opendal_core::{Configurator, Error, ErrorKind, OperatorUri, Result};

/// Config for GitHub services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GithubConfig {
    /// root of this backend.
    ///
    /// All operations will happen under this root.
    pub root: Option<String>,
    /// GitHub access_token.
    ///
    /// optional.
    /// If not provided, the backend will only support read operations for public repositories.
    /// And rate limit will be limited to 60 requests per hour.
    pub token: Option<String>,
    /// GitHub repo owner.
    ///
    /// required.
    pub owner: String,
    /// GitHub repo name.
    ///
    /// required.
    pub repo: String,
}

impl Debug for GithubConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GithubConfig")
            .field("root", &self.root)
            .field("owner", &self.owner)
            .field("repo", &self.repo)
            .finish_non_exhaustive()
    }
}

impl Configurator for GithubConfig {
    type Builder = GithubBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
        let mut map = uri.options().clone();

        // `github://<owner>/<repo>/<root>` URIs provide owner, repo and root
        // through the URI itself. A bare scheme like `github` (used by
        // `Operator::via_iter`) must fall back to the options so that
        // `OPENDAL_GITHUB_OWNER` / `OPENDAL_GITHUB_REPO` environment variables
        // work like they do for other services.
        if let Some(owner) = uri.name() {
            map.insert("owner".to_string(), owner.to_string());
        }

        if let Some(raw_path) = uri.root() {
            let (repo, remainder) = match raw_path.split_once('/') {
                Some((repo, rest)) => (repo, Some(rest)),
                None => (raw_path, None),
            };

            if !repo.is_empty() {
                map.insert("repo".to_string(), repo.to_string());
            }

            if let Some(rest) = remainder
                && !rest.is_empty()
            {
                map.insert("root".to_string(), rest.to_string());
            }
        }

        // Owner and repository must be provided either via the URI or the
        // options; `#[serde(default)]` would otherwise silently turn a missing
        // field into an empty string.
        if map.get("owner").is_none_or(String::is_empty) {
            return Err(Error::new(ErrorKind::ConfigInvalid, "owner is required")
                .with_context("service", GITHUB_SCHEME));
        }
        if map.get("repo").is_none_or(String::is_empty) {
            return Err(
                Error::new(ErrorKind::ConfigInvalid, "repository is required")
                    .with_context("service", GITHUB_SCHEME),
            );
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        GithubBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_owner_repo_and_root() {
        let uri = OperatorUri::new(
            "github://apache/opendal/src/services",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = GithubConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.owner, "apache".to_string());
        assert_eq!(cfg.repo, "opendal".to_string());
        assert_eq!(cfg.root.as_deref(), Some("src/services"));
    }

    #[test]
    fn from_uri_requires_repository() {
        let uri = OperatorUri::new("github://apache", Vec::<(String, String)>::new()).unwrap();

        assert!(GithubConfig::from_uri(&uri).is_err());
    }

    #[test]
    fn from_uri_sets_owner_repo_and_root_from_options() {
        let uri = OperatorUri::new(
            "github",
            vec![
                ("owner".to_string(), "apache".to_string()),
                ("repo".to_string(), "opendal".to_string()),
                ("root".to_string(), "core/tests/data".to_string()),
            ],
        )
        .unwrap();

        let cfg = GithubConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.owner, "apache".to_string());
        assert_eq!(cfg.repo, "opendal".to_string());
        assert_eq!(cfg.root.as_deref(), Some("core/tests/data"));
    }
}
