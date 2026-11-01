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

use super::backend::HfBuilder;
use super::core::HfDownloadMode;
use super::core::HfRepoType;
use super::core::HfUri;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Debug;

use super::HUGGINGFACE_SCHEME;

/// Configuration for Hugging Face service support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct HfConfig {
    /// Repo type of this backend. Required.
    pub repo_type: Option<HfRepoType>,
    /// Repo id of this backend.
    ///
    /// This is required.
    pub repo_id: Option<String>,
    /// Revision of this backend.
    ///
    /// Default is main.
    pub revision: Option<String>,
    /// Root of this backend. Can be "/path/to/dir".
    ///
    /// Default is "/".
    pub root: Option<String>,
    /// Token of this backend.
    ///
    /// This is optional.
    pub token: Option<String>,
    /// Endpoint of the Hugging Face Hub.
    ///
    /// The default is `https://huggingface.co`.
    pub endpoint: Option<String>,
    /// Download mode. Either `xet` (default) or `http`.
    ///
    /// When unset, the mode is resolved from the `HF_HUB_DISABLE_XET`
    /// environment variable: a non-empty value forces `http`, otherwise it
    /// defaults to `xet`. An explicit value here takes precedence.
    ///
    /// See <https://huggingface.co/docs/huggingface_hub/package_reference/environment_variables#hfhubdisablexet>.
    pub download_mode: Option<HfDownloadMode>,
    /// Enable caching of resolved HTTP download addresses and XET file metadata.
    ///
    /// Defaults to `false`. Set to `true` to share resolve results across readers
    /// on the same backend. Changed files may remain invisible while cached
    /// results are reused. A reader retains XET metadata from its first read for
    /// its lifetime. Create a new reader to resolve the path again when this
    /// option is disabled.
    /// See [`HfBuilder::enable_resolve_cache`] for freshness semantics.
    pub enable_resolve_cache: bool,
    /// Disable loading config from the environment.
    ///
    /// When set, the service ignores `HF_ENDPOINT`, `HF_HUB_DISABLE_XET`,
    /// `HF_TOKEN`, `HF_TOKEN_PATH`, and the `token` file under `HF_HOME`,
    /// `$XDG_CACHE_HOME/huggingface` or `~/.cache/huggingface`.
    pub disable_config_load: bool,
}

impl Debug for HfConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HfConfig")
            .field(
                "repo_type",
                &self.repo_type.as_ref().map(HfRepoType::as_str),
            )
            .field("repo_id", &self.repo_id)
            .field("revision", &self.revision)
            .field("root", &self.root)
            .field("download_mode", &self.download_mode)
            .field("enable_resolve_cache", &self.enable_resolve_cache)
            .field("disable_config_load", &self.disable_config_load)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for HfConfig {
    type Builder = HfBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let opts = uri.options();

        // Reconstruct the full path from authority (name) and root.
        // OperatorUri splits "hf://datasets/user/repo@rev/path" into
        // name="datasets" and root="user/repo@rev/path".
        let mut path = String::new();
        if let Some(name) = uri.name()
            && !name.is_empty()
        {
            path.push_str(name);
        }
        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(root);
        }

        let download_mode = opts
            .get("download_mode")
            .map(|s| HfDownloadMode::parse(s))
            .transpose()?;
        let disable_config_load = opts
            .get("disable_config_load")
            .map(|value| value.parse::<bool>())
            .transpose()
            .map_err(|err| {
                opendal_core::Error::new(
                    opendal_core::ErrorKind::ConfigInvalid,
                    "disable_config_load must be true or false",
                )
                .set_source(err)
            })?
            .unwrap_or_default();

        let enable_resolve_cache = opts
            .get("enable_resolve_cache")
            .map(|value| value.parse::<bool>())
            .transpose()
            .map_err(|err| {
                opendal_core::Error::new(
                    opendal_core::ErrorKind::ConfigInvalid,
                    "enable_resolve_cache must be true or false",
                )
                .set_source(err)
            })?
            .unwrap_or_default();

        if !path.is_empty() {
            // Full URI like "hf://datasets/user/repo@rev/path"
            let parsed = HfUri::parse(&path)?;
            Ok(Self {
                repo_type: Some(parsed.repo.repo_type),
                repo_id: Some(parsed.repo.repo_id),
                revision: parsed.repo.revision,
                root: opts.get("root").cloned(),
                token: opts.get("token").cloned(),
                endpoint: opts.get("endpoint").cloned(),
                download_mode,
                enable_resolve_cache,
                disable_config_load,
            })
        } else {
            // Bare scheme from via_iter, all config is in options.
            let repo_type = opts
                .get("repo_type")
                .ok_or_else(|| {
                    opendal_core::Error::new(
                        opendal_core::ErrorKind::ConfigInvalid,
                        "repo_type is required",
                    )
                    .with_context("service", HUGGINGFACE_SCHEME)
                })
                .and_then(|s| HfRepoType::parse(s))?;
            Ok(Self {
                repo_type: Some(repo_type),
                repo_id: opts.get("repo_id").cloned(),
                revision: opts.get("revision").cloned(),
                root: opts.get("root").cloned(),
                token: opts.get("token").cloned(),
                endpoint: opts.get("endpoint").cloned(),
                download_mode,
                enable_resolve_cache,
                disable_config_load,
            })
        }
    }

    fn into_builder(self) -> Self::Builder {
        HfBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_with_all_components() {
        let uri = OperatorUri::new(
            "hf://datasets/username/my_dataset@dev/train/data.csv",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = HfConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.repo_type, Some(HfRepoType::Dataset));
        assert_eq!(cfg.repo_id.as_deref(), Some("username/my_dataset"));
        assert_eq!(cfg.revision.as_deref(), Some("dev"));
        assert!(cfg.root.is_none());
    }

    #[test]
    fn from_uri_via_iter_options() {
        // Simulates the via_iter path: bare scheme with options map.
        let uri = OperatorUri::new(
            "huggingface",
            vec![
                ("repo_type".to_string(), "dataset".to_string()),
                (
                    "repo_id".to_string(),
                    "opendal/huggingface-testdata".to_string(),
                ),
                ("revision".to_string(), "main".to_string()),
                ("root".to_string(), "/testdata/".to_string()),
            ],
        )
        .unwrap();

        let cfg = HfConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.repo_type, Some(HfRepoType::Dataset));
        assert_eq!(cfg.repo_id.as_deref(), Some("opendal/huggingface-testdata"));
        assert_eq!(cfg.revision.as_deref(), Some("main"));
        assert_eq!(cfg.root.as_deref(), Some("/testdata/"));
    }

    #[test]
    fn from_uri_disable_config_load() {
        let parse = |value: &str| {
            let uri = OperatorUri::new(
                "huggingface",
                [
                    ("repo_type".to_string(), "dataset".to_string()),
                    ("disable_config_load".to_string(), value.to_string()),
                ],
            )
            .unwrap();
            HfConfig::from_uri(&uri).map(|cfg| cfg.disable_config_load)
        };

        assert!(parse("true").unwrap());
        assert!(!parse("false").unwrap());
        assert!(parse("yes").is_err());
    }

    #[test]
    fn from_uri_download_mode_http() {
        let uri = OperatorUri::new(
            "huggingface",
            vec![
                ("repo_type".to_string(), "dataset".to_string()),
                ("repo_id".to_string(), "user/repo".to_string()),
                ("download_mode".to_string(), "http".to_string()),
            ],
        )
        .unwrap();

        let cfg = HfConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.download_mode, Some(HfDownloadMode::Http));
    }

    #[test]
    fn from_uri_download_mode_defaults_to_xet() {
        let uri = OperatorUri::new(
            "huggingface",
            vec![
                ("repo_type".to_string(), "model".to_string()),
                ("repo_id".to_string(), "user/repo".to_string()),
            ],
        )
        .unwrap();

        let cfg = HfConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.download_mode.unwrap_or_default(), HfDownloadMode::Xet);
    }

    #[test]
    fn from_uri_resolve_cache_options() {
        for scheme in ["hf://datasets/user/repo", "huggingface"] {
            for (value, expected) in [(None, false), (Some("false"), false), (Some("true"), true)] {
                let mut options = vec![("repo_type", "dataset"), ("repo_id", "user/repo")];
                if let Some(value) = value {
                    options.push(("enable_resolve_cache", value));
                }
                let uri = OperatorUri::new(
                    scheme,
                    options
                        .into_iter()
                        .map(|(key, value)| (key.to_string(), value.to_string())),
                )
                .unwrap();
                assert_eq!(
                    HfConfig::from_uri(&uri).unwrap().enable_resolve_cache,
                    expected
                );
            }

            let uri = OperatorUri::new(
                scheme,
                [("enable_resolve_cache".to_string(), "invalid".to_string())],
            )
            .unwrap();
            let err = HfConfig::from_uri(&uri).unwrap_err();
            assert_eq!(err.kind(), opendal_core::ErrorKind::ConfigInvalid);
            assert!(
                err.to_string()
                    .contains("enable_resolve_cache must be true or false")
            );
        }
    }
}
