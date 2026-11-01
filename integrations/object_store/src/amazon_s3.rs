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

use crate::OpendalStore;
use crate::utils::format_object_store_error;
use object_store::aws::{AmazonS3Builder, AmazonS3ConfigKey};
use opendal::Operator;
use opendal::services::S3;

impl OpendalStore {
    /// Create OpendalStore from object_store Amazon S3 builder.
    pub fn new_amazon_s3(builder: AmazonS3Builder) -> object_store::Result<OpendalStore> {
        let mut s3 = S3::default();
        if let Some(endpoint) = builder.get_config_value(&AmazonS3ConfigKey::Endpoint) {
            s3 = s3.endpoint(endpoint.as_str());
        }
        if let Some(region) = builder.get_config_value(&AmazonS3ConfigKey::Region) {
            s3 = s3.region(region.as_str());
        }
        if let Some(bucket_name) = builder.get_config_value(&AmazonS3ConfigKey::Bucket) {
            s3 = s3.bucket(bucket_name.as_str());
        }
        if let Some(access_key_id) = builder.get_config_value(&AmazonS3ConfigKey::AccessKeyId) {
            s3 = s3.access_key_id(access_key_id.as_str());
        }
        if let Some(secret_access_key) =
            builder.get_config_value(&AmazonS3ConfigKey::SecretAccessKey)
        {
            s3 = s3.secret_access_key(secret_access_key.as_str());
        }
        if let Some(token) = builder.get_config_value(&AmazonS3ConfigKey::Token) {
            s3 = s3.session_token(token.as_str());
        }
        if let Some(virtual_hosted_style_request) =
            builder.get_config_value(&AmazonS3ConfigKey::VirtualHostedStyleRequest)
        {
            let r = virtual_hosted_style_request
                .parse::<bool>()
                .map_err(|err| object_store::Error::Generic {
                    store: "s3",
                    source: Box::new(err),
                })?;
            if r {
                s3 = s3.enable_virtual_host_style();
            }
        }
        if let Some(skip_signature) = builder.get_config_value(&AmazonS3ConfigKey::SkipSignature) {
            let r = skip_signature
                .parse::<bool>()
                .map_err(|err| object_store::Error::Generic {
                    store: "s3",
                    source: Box::new(err),
                })?;
            if r {
                s3 = s3.skip_signature();
            }
        }

        let op = Operator::new(s3).map_err(|err| format_object_store_error(err, ""))?;
        Ok(OpendalStore::new(op))
    }
}
