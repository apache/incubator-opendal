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

use super::backend::GdriveBuilder;

/// [GoogleDrive](https://drive.google.com/) configuration.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GdriveConfig {
    /// The root for gdrive
    pub root: Option<String>,
    /// Access token for gdrive.
    pub access_token: Option<String>,
    /// Refresh token for gdrive.
    pub refresh_token: Option<String>,
    /// Client id for gdrive.
    pub client_id: Option<String>,
    /// Client secret for gdrive.
    pub client_secret: Option<String>,
    /// Shared drive (formerly Team Drive) id for gdrive.
    ///
    /// When set, gdrive resolves `root` inside this shared drive instead of
    /// the user's My Drive.
    pub drive_id: Option<String>,
}

impl Debug for GdriveConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GdriveConfig")
            .field("root", &self.root)
            .field("drive_id", &self.drive_id)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for GdriveConfig {
    type Builder = GdriveBuilder;

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
        GdriveBuilder { config: self }
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
            "gdrive://service/root/folder",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = GdriveConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.root.as_deref(), Some("root/folder"));
    }

    #[test]
    fn from_uri_sets_drive_id() {
        let uri = OperatorUri::new(
            "gdrive://service/root/folder?drive_id=0ABCdefGHIjkl",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = GdriveConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.root.as_deref(), Some("root/folder"));
        assert_eq!(cfg.drive_id.as_deref(), Some("0ABCdefGHIjkl"));
    }
}
