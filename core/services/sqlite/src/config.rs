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

use super::backend::SqliteBuilder;

/// Config for Sqlite support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct SqliteConfig {
    /// Set the connection_string of the sqlite service.
    ///
    /// This connection string is used to connect to the sqlite service.
    ///
    /// The format of connect string resembles the url format of the sqlite client:
    ///
    /// - `sqlite::memory:`
    /// - `sqlite:data.db`
    /// - `sqlite://data.db`
    ///
    /// For more information, please visit <https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html>.
    pub connection_string: Option<String>,

    /// Set the table name of the sqlite service to read/write.
    pub table: Option<String>,
    /// Set the key field name of the sqlite service to read/write.
    ///
    /// Default to `key` if not specified.
    pub key_field: Option<String>,
    /// Set the value field name of the sqlite service to read/write.
    ///
    /// Default to `value` if not specified.
    pub value_field: Option<String>,
    /// set the working directory, all operations will be performed under it.
    ///
    /// default: "/"
    pub root: Option<String>,
}

impl Debug for SqliteConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteConfig")
            .field("table", &self.table)
            .field("key_field", &self.key_field)
            .field("value_field", &self.value_field)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for SqliteConfig {
    type Builder = SqliteBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(authority) = uri.authority() {
            map.entry("connection_string".to_string())
                .or_insert_with(|| format!("sqlite://{authority}"));
        }

        if let Some(path) = uri.root()
            && !path.is_empty()
        {
            let (table, rest) = match path.split_once('/') {
                Some((table, remainder)) => (table, Some(remainder)),
                None => (path, None),
            };

            if !table.is_empty() {
                map.entry("table".to_string())
                    .or_insert_with(|| table.to_string());
            }

            if let Some(root) = rest
                && !root.is_empty()
            {
                map.insert("root".to_string(), root.to_string());
            }
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        SqliteBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_connection_string_table_and_root() {
        let uri =
            OperatorUri::new("sqlite://data.db/kv/cache", Vec::<(String, String)>::new()).unwrap();

        let cfg = SqliteConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.connection_string.as_deref(), Some("sqlite://data.db"));
        assert_eq!(cfg.table.as_deref(), Some("kv"));
        assert_eq!(cfg.root.as_deref(), Some("cache"));
    }
}
