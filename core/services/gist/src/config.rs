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

use super::GIST_SCHEME;
use super::backend::GistBuilder;
use opendal_core::{Configurator, Error, ErrorKind, OperatorUri, Result};

/// Config for GitHub Gist services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GistConfig {
    /// GitHub access token.
    ///
    /// optional.
    /// Without a token this service can only read public gists and is limited to
    /// 60 requests per hour. Updating files requires a token with the `gist` scope.
    pub token: Option<String>,
    /// Identifier of the gist this service operates on.
    ///
    /// required.
    pub gist_id: String,
}

impl Debug for GistConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GistConfig")
            .field("gist_id", &self.gist_id)
            .finish_non_exhaustive()
    }
}

impl Configurator for GistConfig {
    type Builder = GistBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
        let mut map = uri.options().clone();

        // `gist://<gist_id>` carries the identifier in the URI authority. A bare
        // scheme like `gist` (used by `Operator::via_iter`) must fall back to the
        // options so that `OPENDAL_GIST_GIST_ID` keeps working.
        if let Some(gist_id) = uri.name() {
            map.insert("gist_id".to_string(), gist_id.to_string());
        }

        // A gist is a flat set of files, so a URI path cannot select anything.
        if uri.root().is_some_and(|root| !root.is_empty()) {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "gist has no directory tree, remove the path from the URI",
            )
            .with_context("service", GIST_SCHEME));
        }

        // `#[serde(default)]` would otherwise turn a missing identifier into an
        // empty string.
        if map.get("gist_id").is_none_or(String::is_empty) {
            return Err(Error::new(ErrorKind::ConfigInvalid, "gist_id is required")
                .with_context("service", GIST_SCHEME));
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        GistBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_gist_id() {
        let uri = OperatorUri::new(
            "gist://aa5a315d61ae9438b18d",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = GistConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.gist_id, "aa5a315d61ae9438b18d");
    }

    #[test]
    fn from_uri_sets_gist_id_from_options() {
        let uri = OperatorUri::new(
            "gist",
            vec![("gist_id".to_string(), "aa5a315d61ae9438b18d".to_string())],
        )
        .unwrap();

        let cfg = GistConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.gist_id, "aa5a315d61ae9438b18d");
    }

    #[test]
    fn from_uri_requires_gist_id() {
        let uri = OperatorUri::new("gist", Vec::<(String, String)>::new()).unwrap();

        assert!(GistConfig::from_uri(&uri).is_err());
    }

    #[test]
    fn from_uri_rejects_path() {
        let uri = OperatorUri::new(
            "gist://aa5a315d61ae9438b18d/sub",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        assert!(GistConfig::from_uri(&uri).is_err());
    }
}
