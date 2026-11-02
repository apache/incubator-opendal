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

use super::backend::MysqlBuilder;

/// Config for Mysql services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct MysqlConfig {
    /// This connection string is used to connect to the mysql service. There are url based formats.
    ///
    /// The format of connect string resembles the url format of the mysql client.
    /// The format is: `[scheme://][user[:[password]]@]host[:port][/schema][?attribute1=value1&attribute2=value2...`
    ///
    /// - `mysql://user@localhost`
    /// - `mysql://user:password@localhost`
    /// - `mysql://user:password@localhost:3306`
    /// - `mysql://user:password@localhost:3306/db`
    ///
    /// For more information, please refer to <https://docs.rs/sqlx/latest/sqlx/mysql/struct.MySqlConnectOptions.html>.
    pub connection_string: Option<String>,

    /// The table name for mysql.
    pub table: Option<String>,
    /// The key field name for mysql.
    pub key_field: Option<String>,
    /// The value field name for mysql.
    pub value_field: Option<String>,
    /// The root for mysql.
    pub root: Option<String>,
}

impl Debug for MysqlConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MysqlConfig")
            .field("root", &self.root)
            .field("table", &self.table)
            .field("key_field", &self.key_field)
            .field("value_field", &self.value_field)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for MysqlConfig {
    type Builder = MysqlBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(authority) = uri.authority() {
            map.entry("connection_string".to_string())
                .or_insert_with(|| format!("mysql://{authority}"));
        }

        if let Some(path) = uri.root()
            && !path.is_empty()
        {
            let (table_segment, rest) = match path.split_once('/') {
                Some((table, remainder)) => (table, Some(remainder)),
                None => (path, None),
            };

            if !table_segment.is_empty() {
                map.entry("table".to_string())
                    .or_insert_with(|| table_segment.to_string());
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
        MysqlBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_connection_string_table_and_root() {
        let uri = OperatorUri::new(
            "mysql://db.example.com:3306/kv/cache",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = MysqlConfig::from_uri(&uri).unwrap();
        assert_eq!(
            cfg.connection_string.as_deref(),
            Some("mysql://db.example.com:3306")
        );
        assert_eq!(cfg.table.as_deref(), Some("kv"));
        assert_eq!(cfg.root.as_deref(), Some("cache"));
    }

    #[test]
    fn from_uri_respects_existing_table() {
        let uri = OperatorUri::new(
            "mysql://db.example.com:3306/users?root=logs",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = MysqlConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.table.as_deref(), Some("users"));
        assert_eq!(cfg.root.as_deref(), Some("logs"));
    }
}
