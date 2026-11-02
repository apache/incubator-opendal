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

use serde::Deserialize;
use serde::Serialize;

use super::backend::RedbBuilder;

/// Config for redb service support.
#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct RedbConfig {
    /// path to the redb data directory.
    pub datadir: Option<String>,
    /// The table name for redb.
    pub table: Option<String>,
    /// The root for redb.
    pub root: Option<String>,
}

impl opendal_core::Configurator for RedbConfig {
    type Builder = RedbBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(path) = uri.root()
            && !path.is_empty()
        {
            map.entry("datadir".to_string())
                .or_insert_with(|| format!("/{path}"));
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        RedbBuilder {
            config: self,
            database: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_datadir_table_and_root() {
        let uri = OperatorUri::new(
            "redb:///tmp/redb?table=op_table&root=cache",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = RedbConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.datadir.as_deref(), Some("/tmp/redb"));
        assert_eq!(cfg.table.as_deref(), Some("op_table"));
        assert_eq!(cfg.root.as_deref(), Some("cache"));
    }
}
