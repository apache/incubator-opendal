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

use super::backend::MongodbBuilder;

/// Config for Mongodb service support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct MongodbConfig {
    /// connection string of this backend
    pub connection_string: Option<String>,
    /// database of this backend
    pub database: Option<String>,
    /// collection of this backend
    pub collection: Option<String>,
    /// root of this backend
    pub root: Option<String>,
    /// key field of this backend
    pub key_field: Option<String>,
    /// value field of this backend
    pub value_field: Option<String>,
}

impl Debug for MongodbConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MongodbConfig")
            .field("database", &self.database)
            .field("collection", &self.collection)
            .field("root", &self.root)
            .field("key_field", &self.key_field)
            .field("value_field", &self.value_field)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for MongodbConfig {
    type Builder = MongodbBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(authority) = uri.authority() {
            map.entry("connection_string".to_string())
                .or_insert_with(|| format!("mongodb://{authority}"));
        }

        if let Some(path) = uri.root()
            && !path.is_empty()
        {
            let mut segments = path.splitn(3, '/');
            if let Some(db) = segments.next()
                && !db.is_empty()
            {
                map.entry("database".to_string())
                    .or_insert_with(|| db.to_string());
            }
            if let Some(collection) = segments.next()
                && !collection.is_empty()
            {
                map.entry("collection".to_string())
                    .or_insert_with(|| collection.to_string());
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
        MongodbBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_connection_string_database_collection_and_root() {
        let uri = OperatorUri::new(
            "mongodb://mongo.internal:27017/analytics/events/session",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = MongodbConfig::from_uri(&uri).unwrap();
        assert_eq!(
            cfg.connection_string.as_deref(),
            Some("mongodb://mongo.internal:27017")
        );
        assert_eq!(cfg.database.as_deref(), Some("analytics"));
        assert_eq!(cfg.collection.as_deref(), Some("events"));
        assert_eq!(cfg.root.as_deref(), Some("session"));
    }
}
