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

use super::backend::DbfsBuilder;

/// [Dbfs](https://docs.databricks.com/api/azure/workspace/dbfs)'s REST API support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DbfsConfig {
    /// The root for dbfs.
    pub root: Option<String>,
    /// The endpoint for dbfs.
    pub endpoint: Option<String>,
    /// The token for dbfs.
    pub token: Option<String>,
}

impl Debug for DbfsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbfsConfig")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for DbfsConfig {
    type Builder = DbfsBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("endpoint".to_string(), format!("https://{authority}"));
        }

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        DbfsBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_endpoint_and_root() {
        let uri = OperatorUri::new(
            "dbfs://adb-1234567.azuredatabricks.net/api/2.0/dbfs/root",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = DbfsConfig::from_uri(&uri).unwrap();
        assert_eq!(
            cfg.endpoint.as_deref(),
            Some("https://adb-1234567.azuredatabricks.net")
        );
        assert_eq!(cfg.root.as_deref(), Some("api/2.0/dbfs/root"));
    }
}
