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
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

/// Config for MemCached services support
#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(default)]
#[non_exhaustive]
pub struct MemcachedConfig {
    /// network address of the memcached service.
    ///
    /// For example: "tcp://localhost:11211"
    pub endpoint: Option<String>,
    /// the working directory of the service. Can be "/path/to/dir"
    ///
    /// default is "/"
    pub root: Option<String>,
    /// Memcached username, optional.
    pub username: Option<String>,
    /// Memcached password, optional.
    pub password: Option<String>,
    /// The default ttl for put operations.
    pub default_ttl: Option<Duration>,
    /// Enable TLS for the connection.
    ///
    /// Required for AWS ElastiCache Memcached serverless instances.
    pub enable_tls: Option<bool>,
    /// Path to CA certificate file for TLS verification.
    pub ca_cert: Option<String>,
}
