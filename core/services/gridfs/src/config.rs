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

use opendal_core::*;
use serde::Deserialize;
use serde::Serialize;

use super::backend::GridfsBuilder;

/// Config for Grid file system support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GridfsConfig {
    /// The connection string of the MongoDB service.
    pub connection_string: Option<String>,
    /// The database name of the MongoDB GridFs service to read/write.
    pub database: Option<String>,
    /// The bucket name of the MongoDB GridFs service to read/write.
    pub bucket: Option<String>,
    /// The chunk size of the MongoDB GridFs service used to break the user file into chunks.
    pub chunk_size: Option<u32>,
    /// The working directory, all operations will be performed under it.
    pub root: Option<String>,
}

impl Debug for GridfsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridFsConfig")
            .field("database", &self.database)
            .field("bucket", &self.bucket)
            .field("chunk_size", &self.chunk_size)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Configurator for GridfsConfig {
    type Builder = GridfsBuilder;

    fn from_uri(uri: &OperatorUri) -> Result<Self> {
        let mut map = uri.options().clone();

        if let Some(authority) = uri.authority() {
            map.entry("connection_string".to_string())
                .or_insert_with(|| format!("mongodb://{authority}"));
        }

        if let Some(path) = uri.root()
            && !path.is_empty()
        {
            let mut segments = path.splitn(3, '/');
            if let Some(database) = segments.next()
                && !database.is_empty()
            {
                map.entry("database".to_string())
                    .or_insert_with(|| database.to_string());
            }
            if let Some(bucket) = segments.next()
                && !bucket.is_empty()
            {
                map.entry("bucket".to_string())
                    .or_insert_with(|| bucket.to_string());
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
        GridfsBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_uri_sets_connection_database_bucket_and_root() -> Result<()> {
        let uri = OperatorUri::new(
            "gridfs://mongo.example.com:27017/app_files/assets/images",
            Vec::<(String, String)>::new(),
        )?;

        let cfg = GridfsConfig::from_uri(&uri)?;
        assert_eq!(
            cfg.connection_string.as_deref(),
            Some("mongodb://mongo.example.com:27017")
        );
        assert_eq!(cfg.database.as_deref(), Some("app_files"));
        assert_eq!(cfg.bucket.as_deref(), Some("assets"));
        assert_eq!(cfg.root.as_deref(), Some("images"));
        Ok(())
    }
}
