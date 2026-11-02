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

use opendal_core::raw::*;
use serde::Deserialize;
use serde::Serialize;

use super::CLOUDFLARE_KV_SCHEME;
use super::backend::CloudflareKvBuilder;

/// Cloudflare KV Service Support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CloudflareKvConfig {
    /// The token used to authenticate with CloudFlare.
    pub api_token: Option<String>,
    /// The account ID used to authenticate with CloudFlare. Used as URI path parameter.
    pub account_id: Option<String>,
    /// The namespace ID. Used as URI path parameter.
    pub namespace_id: Option<String>,
    /// The default ttl for write operations.
    pub default_ttl: Option<SignedDuration>,

    /// Root within this backend.
    pub root: Option<String>,
}

impl Debug for CloudflareKvConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudflareKvConfig")
            .field("account_id", &self.account_id)
            .field("namespace_id", &self.namespace_id)
            .field("default_ttl", &self.default_ttl)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for CloudflareKvConfig {
    type Builder = CloudflareKvBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let account_id = uri.name().ok_or_else(|| {
            opendal_core::Error::new(
                opendal_core::ErrorKind::ConfigInvalid,
                "uri host must contain account id",
            )
            .with_context("service", CLOUDFLARE_KV_SCHEME)
        })?;

        let raw_root = uri.root().ok_or_else(|| {
            opendal_core::Error::new(
                opendal_core::ErrorKind::ConfigInvalid,
                "uri path must contain namespace id",
            )
            .with_context("service", CLOUDFLARE_KV_SCHEME)
        })?;

        let mut segments = raw_root.splitn(2, '/');
        let namespace_id = segments.next().filter(|s| !s.is_empty()).ok_or_else(|| {
            opendal_core::Error::new(
                opendal_core::ErrorKind::ConfigInvalid,
                "namespace id is required in uri path",
            )
            .with_context("service", CLOUDFLARE_KV_SCHEME)
        })?;

        let mut map = uri.options().clone();
        map.insert("account_id".to_string(), account_id.to_string());
        map.insert("namespace_id".to_string(), namespace_id.to_string());

        if let Some(rest) = segments.next()
            && !rest.is_empty()
        {
            map.insert("root".to_string(), rest.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        CloudflareKvBuilder {
            config: self,
            default_ttl: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_extracts_ids_and_root() {
        let uri = OperatorUri::new(
            "cloudflare-kv://acc123/ns456/prefix/dir",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = CloudflareKvConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.account_id.as_deref(), Some("acc123"));
        assert_eq!(cfg.namespace_id.as_deref(), Some("ns456"));
        assert_eq!(cfg.root.as_deref(), Some("prefix/dir"));
    }

    #[test]
    fn from_uri_requires_namespace() {
        let uri =
            OperatorUri::new("cloudflare-kv://acc123", Vec::<(String, String)>::new()).unwrap();

        assert!(CloudflareKvConfig::from_uri(&uri).is_err());
    }

    #[test]
    fn from_iter_parses_default_ttl() {
        let cfg = CloudflareKvConfig::from_iter([("default_ttl".to_string(), "PT1M".to_string())])
            .unwrap();

        assert_eq!(cfg.default_ttl, Some(SignedDuration::from_mins(1)));
    }
}
