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

use crate::utils::format_object_store_error;
use crate::OpendalStore;
use opendal::services::S3;
use opendal::Operator;

pub struct AmazonS3Builder {
    builder: S3,
}

impl Default for AmazonS3Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl AmazonS3Builder {
    pub fn new() -> Self {
        AmazonS3Builder {
            builder: S3::default(),
        }
    }

    pub fn root(mut self, root_path: impl Into<String>) -> Self {
        self.builder = self.builder.root(root_path.into().as_str());
        self
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.builder = self.builder.endpoint(endpoint.into().as_str());
        self
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.builder = self.builder.region(region.into().as_str());
        self
    }

    pub fn with_bucket_name(mut self, bucket_name: impl Into<String>) -> Self {
        self.builder = self.builder.bucket(bucket_name.into().as_str());
        self
    }

    pub fn with_access_key_id(mut self, access_key_id: impl Into<String>) -> Self {
        self.builder = self.builder.access_key_id(access_key_id.into().as_str());
        self
    }

    pub fn with_secret_access_key(mut self, secret_access_key: impl Into<String>) -> Self {
        self.builder = self
            .builder
            .secret_access_key(secret_access_key.into().as_str());
        self
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.builder = self.builder.session_token(token.into().as_str());
        self
    }

    pub fn with_virtual_hosted_style_request(mut self, virtual_hosted_style_request: bool) -> Self {
        if virtual_hosted_style_request {
            self.builder = self.builder.enable_virtual_host_style();
        }
        self
    }

    pub fn with_skip_signature(mut self, skip_signature: bool) -> Self {
        if skip_signature {
            self.builder = self.builder.allow_anonymous();
        }
        self
    }
    pub fn build(self) -> object_store::Result<OpendalStore> {
        let op = Operator::new(self.builder)
            .map_err(|err| format_object_store_error(err, ""))?
            .finish();

        Ok(OpendalStore::new(op))
    }
}
