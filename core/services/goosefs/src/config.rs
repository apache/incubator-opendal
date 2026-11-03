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

use super::backend::GoosefsBuilder;

/// Config for GooseFS service support.
///
/// GooseFS is a distributed caching file system,
/// accessed via native gRPC protocol (not REST Proxy).
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct GoosefsConfig {
    /// Root path of this backend.
    ///
    /// All operations will happen under this root.
    /// Default to `/` if not set.
    pub root: Option<String>,

    /// Master address(es) in `host:port` format.
    ///
    /// For single master: `"10.0.0.1:9200"`
    /// For HA (comma-separated): `"10.0.0.1:9200,10.0.0.2:9200,10.0.0.3:9200"`
    ///
    /// When multiple addresses are provided, the client uses
    /// `PollingMasterInquireClient` to discover the Primary Master automatically.
    ///
    /// Resolution precedence at `build()` time (highest → lowest), following
    /// [goosefs-sdk `docs/CLIENT_CONFIGURATION.md`](https://github.com/Tencent/tencent-goosefs-rust-sdk/blob/main/docs/CLIENT_CONFIGURATION.md) §1:
    ///   1. `GOOSEFS_MASTER_ADDR` environment variable
    ///   2. `goosefs.master.rpc.addresses` / `goosefs.master.hostname` in
    ///      `goosefs-site.properties`
    ///   3. This field (from the builder, the OpenDAL config map, or the URI
    ///      authority of `goosefs://host:port/path`)
    ///
    /// A site file that declares masters therefore outranks this field: the
    /// file carries the deployment's whole HA master list, which a single URI
    /// authority cannot express. Set `GOOSEFS_MASTER_ADDR` to override a
    /// deployed site file for one process.
    ///
    /// `build()` fails with `ConfigInvalid` when **none** of the above
    /// supplies a master address.
    pub master_addr: Option<String>,

    /// Block size in bytes for new files (default: 64 MiB).
    pub block_size: Option<u64>,

    /// Chunk size in bytes for streaming RPCs (default: 1 MiB).
    pub chunk_size: Option<u64>,

    /// Default write type for new files.
    ///
    /// Supported values: `"must_cache"`, `"try_cache"`, `"cache_through"`,
    /// `"through"`, `"async_through"`. Matching is case-insensitive.
    /// `build()` fails with `ConfigInvalid` when the value is not one of
    /// these.
    /// Default: `"must_cache"`.
    pub write_type: Option<String>,

    /// Authentication type.
    ///
    /// Supported values: `"nosasl"`, `"simple"`.
    /// Default: `"simple"` — PLAIN SASL with username.
    /// `"nosasl"` — skip authentication entirely.
    pub auth_type: Option<String>,

    /// Authentication username.
    ///
    /// Used in SIMPLE mode as the login identity.
    /// Default: current OS user (`$USER` / `$USERNAME`).
    pub auth_username: Option<String>,
}

impl Debug for GoosefsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoosefsConfig")
            .field("root", &self.root)
            .field("master_addr", &self.master_addr)
            .field("block_size", &self.block_size)
            .field("chunk_size", &self.chunk_size)
            .field("write_type", &self.write_type)
            .field("auth_type", &self.auth_type)
            .field("auth_username", &self.auth_username)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for GoosefsConfig {
    type Builder = GoosefsBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();
        if let Some(authority) = uri.authority() {
            // goosefs://host:port/path → master_addr = "host:port"
            map.insert("master_addr".to_string(), authority.to_string());
        }
        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }
        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        GoosefsBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    /// `goosefs://host:port/path` must map to `master_addr=host:port` and
    /// `root=<path without surrounding slashes>`. This mirrors the behavior
    /// OpenDAL's `Operator::from_uri("goosefs://...")` relies on and is
    /// exercised indirectly by the behavior test matrix through
    /// `Operator::via_iter("goosefs", ...)`.
    #[test]
    fn from_uri_sets_master_addr_and_root() {
        let uri = OperatorUri::new(
            "goosefs://10.0.0.1:9200/data/raw",
            Vec::<(String, String)>::new(),
        )
        .expect("valid uri");

        let cfg = GoosefsConfig::from_uri(&uri).expect("from_uri should succeed");

        assert_eq!(cfg.master_addr.as_deref(), Some("10.0.0.1:9200"));
        // `OperatorUri::new` trims leading/trailing slashes from the path, so
        // the root stored here is the *inner* part only. Normalization to an
        // absolute "/…/…/" form happens inside `GoosefsBuilder::build()`.
        assert_eq!(cfg.root.as_deref(), Some("data/raw"));
    }

    /// When the URI path is empty (or just `/`), `root` must stay `None` and
    /// `build()` is expected to fall back to the builder-level default.
    #[test]
    fn from_uri_without_path_leaves_root_none() {
        let uri = OperatorUri::new("goosefs://master:9200", Vec::<(String, String)>::new())
            .expect("valid uri");

        let cfg = GoosefsConfig::from_uri(&uri).expect("from_uri should succeed");

        assert_eq!(cfg.master_addr.as_deref(), Some("master:9200"));
        assert!(
            cfg.root.is_none(),
            "empty path must not produce a root, got: {:?}",
            cfg.root
        );
    }

    /// HA note: since URIs don't allow commas in `host`, HA mode is
    /// expressed through the extra-option/env-var pathway, not through the
    /// URI authority. When both are present the URI authority **wins** the
    /// `master_addr` slot — a user who typed `goosefs://host:port/` must not
    /// be silently redirected by a stale option map. The full HA list
    /// therefore reaches `build()` only when the caller goes through
    /// `from_iter` / `Operator::via_iter` without the URI authority, which is
    /// covered by [`from_iter_picks_up_every_known_field`], or through
    /// `goosefs-site.properties` / `GOOSEFS_MASTER_ADDR`, which outrank
    /// `master_addr` inside `build()`.
    #[test]
    fn from_uri_authority_overrides_extra_master_addr() {
        let uri = OperatorUri::new(
            "goosefs://host1:9200/",
            vec![(
                "master_addr".into(),
                "host1:9200,host2:9200,host3:9200".into(),
            )],
        )
        .expect("valid uri");

        let cfg = GoosefsConfig::from_uri(&uri).expect("from_uri should succeed");

        assert_eq!(
            cfg.master_addr.as_deref(),
            Some("host1:9200"),
            "URI authority must win over extra-options master_addr to \
             prevent stale env/option values from silently shadowing the \
             user-typed URI"
        );
    }

    /// `from_iter` is the path taken by `Operator::via_iter(scheme, env_map)`
    /// — the main entry point used by the behavior test harness (it reads
    /// `OPENDAL_GOOSEFS_*` env vars into a HashMap).
    #[test]
    fn from_iter_picks_up_every_known_field() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert("root".into(), "/tmp/opendal/".into());
        map.insert("master_addr".into(), "127.0.0.1:9200".into());
        map.insert("write_type".into(), "cache_through".into());
        map.insert("auth_type".into(), "simple".into());
        map.insert("auth_username".into(), "opendal".into());

        let cfg = GoosefsConfig::from_iter(map).expect("from_iter should succeed");

        assert_eq!(cfg.root.as_deref(), Some("/tmp/opendal/"));
        assert_eq!(cfg.master_addr.as_deref(), Some("127.0.0.1:9200"));
        assert_eq!(cfg.write_type.as_deref(), Some("cache_through"));
        assert_eq!(cfg.auth_type.as_deref(), Some("simple"));
        assert_eq!(cfg.auth_username.as_deref(), Some("opendal"));
    }
}
