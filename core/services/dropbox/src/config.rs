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

use super::backend::DropboxBuilder;

/// Config for [Dropbox](https://www.dropbox.com/) backend support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct DropboxConfig {
    /// root path for dropbox.
    pub root: Option<String>,
    /// access token for dropbox.
    pub access_token: Option<String>,
    /// refresh_token for dropbox.
    pub refresh_token: Option<String>,
    /// client_id for dropbox.
    pub client_id: Option<String>,
    /// client_secret for dropbox.
    pub client_secret: Option<String>,
}

impl Debug for DropboxConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropBoxConfig")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for DropboxConfig {
    type Builder = DropboxBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        DropboxBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_root() {
        let uri =
            OperatorUri::new("dropbox://remote/documents", Vec::<(String, String)>::new()).unwrap();

        let cfg = DropboxConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.root.as_deref(), Some("documents"));
    }
}
