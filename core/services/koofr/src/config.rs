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

use super::KOOFR_SCHEME;
use super::backend::KoofrBuilder;

/// Config for Koofr services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct KoofrConfig {
    /// root of this backend.
    ///
    /// All operations will happen under this root.
    pub root: Option<String>,
    /// Koofr endpoint.
    pub endpoint: String,
    /// Koofr email.
    pub email: String,
    /// password of this backend. (Must be the application password)
    pub password: Option<String>,
}

impl Debug for KoofrConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KoofrConfig")
            .field("root", &self.root)
            .field("email", &self.email)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for KoofrConfig {
    type Builder = KoofrBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let raw_path = uri.root().ok_or_else(|| {
            opendal_core::Error::new(
                opendal_core::ErrorKind::ConfigInvalid,
                "uri path must contain email",
            )
            .with_context("service", KOOFR_SCHEME)
        })?;

        let mut segments = raw_path.splitn(2, '/');
        let email = segments.next().filter(|s| !s.is_empty()).ok_or_else(|| {
            opendal_core::Error::new(
                opendal_core::ErrorKind::ConfigInvalid,
                "email is required in uri path",
            )
            .with_context("service", KOOFR_SCHEME)
        })?;

        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), format!("https://{authority}"));
        }
        map.insert("email".to_string(), email.to_string());

        if let Some(rest) = segments.next()
            && !rest.is_empty()
        {
            map.insert("root".to_string(), rest.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        KoofrBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_endpoint_email_and_root() {
        let uri = OperatorUri::new(
            "koofr://api.koofr.net/me%40example.com/library",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = KoofrConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoint, "https://api.koofr.net".to_string());
        assert_eq!(cfg.email, "me@example.com".to_string());
        assert_eq!(cfg.root.as_deref(), Some("library"));
    }

    #[test]
    fn from_uri_requires_email_segment() {
        let uri =
            OperatorUri::new("koofr://api.koofr.net", Vec::<(String, String)>::new()).unwrap();

        assert!(KoofrConfig::from_uri(&uri).is_err());
    }
}
