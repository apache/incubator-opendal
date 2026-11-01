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

use super::backend::EtcdBuilder;

/// Config for Etcd services support.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct EtcdConfig {
    /// network address of the Etcd services.
    /// Use HTTPS endpoints for TLS. Set `ca_path` to trust a custom certificate authority.
    /// Client authentication additionally requires both `cert_path` and `key_path`.
    /// e.g. "127.0.0.1:23790,127.0.0.1:23791,127.0.0.1:23792" or "http://127.0.0.1:23790,http://127.0.0.1:23791,http://127.0.0.1:23792" or "https://127.0.0.1:23790,https://127.0.0.1:23791,https://127.0.0.1:23792"
    ///
    /// default is "http://127.0.0.1:2379"
    pub endpoints: Option<String>,
    /// the username to connect etcd service.
    ///
    /// default is None
    pub username: Option<String>,
    /// the password for authentication
    ///
    /// default is None
    pub password: Option<String>,
    /// the working directory of the etcd service. Can be "/path/to/dir"
    ///
    /// default is "/"
    pub root: Option<String>,
    /// PEM certificate authority file used to verify the server.
    /// Enables TLS independently of `cert_path` and `key_path`.
    ///
    /// default is None
    pub ca_path: Option<String>,
    /// PEM client certificate file used for mutual TLS.
    /// Must be configured together with `key_path`.
    ///
    /// default is None
    pub cert_path: Option<String>,
    /// PEM client private key file used for mutual TLS.
    /// Must be configured together with `cert_path`.
    ///
    /// default is None
    pub key_path: Option<String>,
    /// the maximum size in bytes of a gRPC response that the etcd client decodes.
    ///
    /// Set this to read values larger than the etcd client's own limit. The etcd
    /// server must also allow responses of that size.
    ///
    /// Refer to [`etcd_client::KvClient::max_decoding_message_size`](https://docs.rs/etcd-client/latest/etcd_client/struct.KvClient.html#method.max_decoding_message_size)
    ///
    /// default is None, which keeps the etcd client default of 4 MiB
    pub max_decoding_message_size: Option<usize>,
}

impl Debug for EtcdConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EtcdConfig")
            .field("root", &self.root)
            .field("endpoints", &self.endpoints)
            .field("username", &self.username)
            .field("ca_path", &self.ca_path)
            .field("cert_path", &self.cert_path)
            .field("key_path", &self.key_path)
            .field("max_decoding_message_size", &self.max_decoding_message_size)
            .finish_non_exhaustive()
    }
}

impl opendal_core::Configurator for EtcdConfig {
    type Builder = EtcdBuilder;

    fn from_uri(uri: &opendal_core::OperatorUri) -> opendal_core::Result<Self> {
        let mut map = uri.options().clone();

        if let Some(authority) = uri.authority() {
            map.entry("endpoints".to_string())
                .or_insert_with(|| format!("http://{authority}"));
        }

        if let Some(root) = uri.root()
            && !root.is_empty()
        {
            map.insert("root".to_string(), root.to_string());
        }

        Self::from_iter(map)
    }

    fn into_builder(self) -> Self::Builder {
        EtcdBuilder { config: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Configurator;
    use opendal_core::OperatorUri;

    #[test]
    fn from_uri_sets_endpoints_and_root() {
        let uri = OperatorUri::new(
            "etcd://127.0.0.1:2379/app/config",
            Vec::<(String, String)>::new(),
        )
        .unwrap();

        let cfg = EtcdConfig::from_uri(&uri).unwrap();
        assert_eq!(cfg.endpoints.as_deref(), Some("http://127.0.0.1:2379"));
        assert_eq!(cfg.root.as_deref(), Some("app/config"));
    }
}
