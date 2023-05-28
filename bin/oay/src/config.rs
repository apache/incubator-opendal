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

use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub backend: BackendConfig,
    pub frontends: FrontendsConfig,
}

#[derive(Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub typ: String,
}

#[derive(Serialize, Deserialize)]
pub struct FrontendsConfig {
    pub s3: S3Config,
}

#[derive(Serialize, Deserialize)]
pub struct S3Config {
    pub enable: bool,
    pub addr: String,
    #[serde(default = "1000")]
    pub batch_max_operations: usize,
}
