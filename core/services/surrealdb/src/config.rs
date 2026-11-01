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

use super::backend::SurrealdbBuilder;

/// Config for Surrealdb services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct SurrealdbConfig {
    /// The connection string for surrealdb.
    pub connection_string: Option<String>,
    /// The username for surrealdb.
    pub username: Option<String>,
    /// The password for surrealdb.
    pub password: Option<String>,
    /// The namespace for surrealdb.
    pub namespace: Option<String>,
    /// The database for surrealdb.
    pub database: Option<String>,
    /// The table for surrealdb.
    pub table: Option<String>,
    /// The key field for surrealdb.
    pub key_field: Option<String>,
    /// The value field for surrealdb.
    pub value_field: Option<String>,
    /// The root for surrealdb.
    pub root: Option<String>,
}

impl Debug for SurrealdbConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurrealdbConfig")
            .field("username", &self.username)
            .field("namespace", &self.namespace)
            .field("database", &self.database)
            .field("table", &self.table)
            .field("key_field", &self.key_field)
            .field("value_field", &self.value_field)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for SurrealdbConfig {
    type Builder = SurrealdbBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(authority) = uri.authority() {
            map.entry("connection_string".to_string())
                .or_insert_with(|| format!("ws://{authority}"));
        }

        if let Some(path) = uri.root()
            && !path.is_empty()
        {
            let mut segments = path.splitn(4, '/');
            if let Some(namespace) = segments.next()
                && !namespace.is_empty()
            {
                map.entry("namespace".to_string())
                    .or_insert_with(|| namespace.to_string());
            }
            if let Some(database) = segments.next()
                && !database.is_empty()
            {
                map.entry("database".to_string())
                    .or_insert_with(|| database.to_string());
            }
            if let Some(table) = segments.next()
                && !table.is_empty()
            {
                map.entry("table".to_string())
                    .or_insert_with(|| table.to_string());
            }
            if let Some(rest) = segments.next()
                && !rest.is_empty()
            {
                map.insert("root".to_string(), rest.to_string());
            }
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        SurrealdbBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_connection_namespace_database_table_and_root() {
        let uri = OperatorUri::new(
            "surrealdb://db.example.com:8000/project/app/cache/static",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = SurrealdbConfig::from_uri(&uri).unwrap();
        assert_eq!(
            cfg.connection_string.as_deref(),
            Some("ws://db.example.com:8000")
        );
        assert_eq!(cfg.namespace.as_deref(), Some("project"));
        assert_eq!(cfg.database.as_deref(), Some("app"));
        assert_eq!(cfg.table.as_deref(), Some("cache"));
        assert_eq!(cfg.root.as_deref(), Some("static"));
    }
}
