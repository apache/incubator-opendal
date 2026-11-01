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

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CfKvResponse {
    pub errors: Vec<CfKvError>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CfKvMetadata {
    pub etag: String,
    pub last_modified: String,
    pub content_length: usize,
    pub is_dir: bool,
}

#[derive(Debug, Deserialize)]
pub struct CfKvError {
    pub code: i32,
}

#[derive(Debug, Deserialize)]
pub struct CfKvDeleteResult {
    pub successful_key_count: usize,
    pub unsuccessful_keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CfKvDeleteResponse {
    pub success: bool,
    pub result: Option<CfKvDeleteResult>,
}

#[derive(Debug, Deserialize)]
pub struct CfKvListKey {
    pub name: String,
    pub metadata: CfKvMetadata,
    // expiration: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CfKvListResultInfo {
    // pub count: Option<usize>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CfKvListResponse {
    pub success: bool,
    pub result_info: Option<CfKvListResultInfo>,
    pub result: Option<Vec<CfKvListKey>>,
}

#[derive(Debug, Deserialize)]
pub struct CfKvStatResponse {
    pub success: bool,
    pub result: Option<CfKvMetadata>,
}
