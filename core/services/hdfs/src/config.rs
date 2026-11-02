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

use super::backend::HdfsBuilder;

/// [Hadoop Distributed File System (HDFS™)](https://hadoop.apache.org/) support.
///
/// Config for Hdfs services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct HdfsConfig {
    /// work dir of this backend
    pub root: Option<String>,
    /// name node of this backend
    pub name_node: Option<String>,
    /// kerberos_ticket_cache_path of this backend
    pub kerberos_ticket_cache_path: Option<String>,
    /// user of this backend
    pub user: Option<String>,
    /// Deprecated: HDFS append capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "HDFS append capability is enabled by default and this option is no longer needed."
    )]
    pub enable_append: bool,
    /// atomic_write_dir of this backend
    pub atomic_write_dir: Option<String>,
}

impl Debug for HdfsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdfsConfig")
            .field("root", &self.root)
            .field("name_node", &self.name_node)
            .field(
                "kerberos_ticket_cache_path",
                &self.kerberos_ticket_cache_path,
            )
            .field("user", &self.user)
            .field("atomic_write_dir", &self.atomic_write_dir)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for HdfsConfig {
    type Builder = HdfsBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            map.insert("name_node".to_string(), format!("hdfs://{authority}"));
        }

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    #[allow(deprecated)]
    fn into_builder(self) -> Self::Builder {
        HdfsBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_name_node_and_root() {
        let uri = OperatorUri::new(
            "hdfs://cluster.local:8020/user/data",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = HdfsConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.name_node.as_deref(), Some("hdfs://cluster.local:8020"));
        assert_eq!(cfg.root.as_deref(), Some("user/data"));
    }

    #[test]
    fn from_uri_allows_missing_authority() {
        let uri = OperatorUri::new("hdfs", Vec::<(String, String)>::new()).unwrap();

        let cfg = HdfsConfig::from_uri(&uri).unwrap();
        assert!(cfg.name_node.is_none());
    }
}
