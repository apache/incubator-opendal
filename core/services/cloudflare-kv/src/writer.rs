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

use std::sync::Arc;

use http::StatusCode;
use opendal_core::raw::*;
use opendal_core::*;

use super::core::parse_error;
use super::core::{CloudflareKvCore, ErrorContext};
use super::model::CfKvMetadata;

pub struct CloudflareWriter {
    core: Arc<CloudflareKvCore>,
    ctx: OperationContext,
    path: String,
}

impl CloudflareWriter {
    pub fn new(core: Arc<CloudflareKvCore>, ctx: OperationContext, path: String) -> Self {
        CloudflareWriter { core, ctx, path }
    }
}

impl oio::OneShotWrite for CloudflareWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let cf_kv_metadata = CfKvMetadata {
            etag: build_tmp_path_of(&self.path),
            last_modified: Timestamp::now().to_string(),
            content_length: bs.len(),
            is_dir: self.path.ends_with('/'),
        };

        let resp = self
            .core
            .set(&self.ctx, &self.path, bs, cf_kv_metadata.clone())
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let mut metadata = MetadataBuilder::unknown();
                metadata.etag(&cf_kv_metadata.etag);
                metadata.last_modified(cf_kv_metadata.last_modified.parse::<Timestamp>()?);
                metadata.set_file(cf_kv_metadata.content_length as u64);

                Ok(metadata.build())
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("WriteValue")),
                resp,
            )),
        }
    }
}
