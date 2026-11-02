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

use super::backend::TikvBuilder;

/// Config for Tikv services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct TikvConfig {
    /// network address of the TiKV service.
    pub endpoints: Option<Vec<String>>,
    /// whether using insecure connection to TiKV
    pub insecure: bool,
    /// certificate authority file path
    pub ca_path: Option<String>,
    /// cert path
    pub cert_path: Option<String>,
    /// key path
    pub key_path: Option<String>,
}

impl Debug for TikvConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TikvConfig")
            .field("endpoints", &self.endpoints)
            .field("insecure", &self.insecure)
            .field("ca_path", &self.ca_path)
            .field("cert_path", &self.cert_path)
            .field("key_path", &self.key_path)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for TikvConfig {
    type Builder = TikvBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let map = uri.options().clone();

        let mut endpoints = Vec::new();
        if let Some(authority) = uri.authority()
            && !authority.is_empty()
        {
            endpoints.push(authority.to_string());
        }

        if let Some(path) = uri.root() {
            for segment in path.split('/') {
                for endpoint in segment.split(',') {
                    let trimmed = endpoint.trim();
                    if !trimmed.is_empty() {
                        endpoints.push(trimmed.to_string());
                    }
                }
            }
        }

        let mut cfg = Self::from_iter(map)?;

        if !endpoints.is_empty() {
            if let Some(existing) = cfg.endpoints.as_mut() {
                existing.extend(endpoints);
            } else {
                cfg.endpoints = Some(endpoints);
            }
        }

        Ok(cfg)
    }

    fn into_builder(self) -> Self::Builder {
        TikvBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_collects_endpoints() {
        let uri = OperatorUri::new(
            "tikv://pd1:2379/pd2:2379,pd3:2379",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = TikvConfig::from_uri(&uri).unwrap();
        assert_eq!(
            cfg.endpoints,
            Some(vec![
                "pd1:2379".to_string(),
                "pd2:2379".to_string(),
                "pd3:2379".to_string()
            ])
        );
    }
}
