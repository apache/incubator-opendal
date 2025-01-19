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

use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Write;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use constants::X_AMZ_META_PREFIX;
use http::Response;
use http::StatusCode;
use log::debug;
use log::warn;
use md5::Digest;
use md5::Md5;
use once_cell::sync::Lazy;
use reqsign::AwsAssumeRoleLoader;
use reqsign::AwsConfig;
use reqsign::AwsCredentialLoad;
use reqsign::AwsDefaultLoader;
use reqsign::AwsV4Signer;
use reqwest::Url;

use super::core::*;
use super::delete::S3Deleter;
use super::error::parse_error;
use super::lister::{S3Lister, S3Listers, S3ObjectVersionsLister};
use super::writer::S3Writer;
use super::writer::S3Writers;
use crate::raw::oio::PageLister;
use crate::raw::*;
use crate::services::S3Config;
use crate::*;
use constants::X_AMZ_VERSION_ID;

/// Allow constructing correct region endpoint if user gives a global endpoint.
static ENDPOINT_TEMPLATES: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    // AWS S3 Service.
    m.insert(
        "https://s3.amazonaws.com",
        "https://s3.{region}.amazonaws.com",
    );
    m
});

const DEFAULT_BATCH_MAX_OPERATIONS: usize = 1000;

impl Configurator for S3Config {
    type Builder = S3Builder;
    fn into_builder(self) -> Self::Builder {
        S3Builder {
            config: self,
            customized_credential_load: None,
            http_client: None,
        }
    }
}

/// Aws S3 and compatible services (including minio, digitalocean space, Tencent Cloud Object Storage(COS) and so on) support.
/// For more information about s3-compatible services, refer to [Compatible Services](#compatible-services).
#[doc = include_str!("docs.md")]
#[doc = include_str!("compatible_services.md")]
#[derive(Default)]
pub struct S3Builder {
    config: S3Config,

    customized_credential_load: Option<Box<dyn AwsCredentialLoad>>,
    http_client: Option<HttpClient>,
}

impl Debug for S3Builder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("S3Builder");

        d.field("config", &self.config);
        d.finish_non_exhaustive()
    }
}

impl S3Builder {
    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set bucket name of this backend.
    pub fn bucket(mut self, bucket: &str) -> Self {
        self.config.bucket = bucket.to_string();

        self
    }

    /// Set endpoint of this backend.
    ///
    /// Endpoint must be full uri, e.g.
    ///
    /// - AWS S3: `https://s3.amazonaws.com` or `https://s3.{region}.amazonaws.com`
    /// - Cloudflare R2: `https://<ACCOUNT_ID>.r2.cloudflarestorage.com`
    /// - Aliyun OSS: `https://{region}.aliyuncs.com`
    /// - Tencent COS: `https://cos.{region}.myqcloud.com`
    /// - Minio: `http://127.0.0.1:9000`
    ///
    /// If user inputs endpoint without scheme like "s3.amazonaws.com", we
    /// will prepend "https://" before it.
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            // Trim trailing `/` so that we can accept `http://127.0.0.1:9000/`
            self.config.endpoint = Some(endpoint.trim_end_matches('/').to_string())
        }

        self
    }

    /// Region represent the signing region of this endpoint. This is required
    /// if you are using the default AWS S3 endpoint.
    ///
    /// If using a custom endpoint,
    /// - If region is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn region(mut self, region: &str) -> Self {
        if !region.is_empty() {
            self.config.region = Some(region.to_string())
        }

        self
    }

    /// Set access_key_id of this backend.
    ///
    /// - If access_key_id is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn access_key_id(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.access_key_id = Some(v.to_string())
        }

        self
    }

    /// Set secret_access_key of this backend.
    ///
    /// - If secret_access_key is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn secret_access_key(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.secret_access_key = Some(v.to_string())
        }

        self
    }

    /// Set role_arn for this backend.
    ///
    /// If `role_arn` is set, we will use already known config as source
    /// credential to assume role with `role_arn`.
    pub fn role_arn(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.role_arn = Some(v.to_string())
        }

        self
    }

    /// Set external_id for this backend.
    pub fn external_id(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.external_id = Some(v.to_string())
        }

        self
    }

    /// Set role_session_name for this backend.
    pub fn role_session_name(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.role_session_name = Some(v.to_string())
        }

        self
    }

    /// Set default storage_class for this backend.
    ///
    /// Available values:
    /// - `DEEP_ARCHIVE`
    /// - `GLACIER`
    /// - `GLACIER_IR`
    /// - `INTELLIGENT_TIERING`
    /// - `ONEZONE_IA`
    /// - `OUTPOSTS`
    /// - `REDUCED_REDUNDANCY`
    /// - `STANDARD`
    /// - `STANDARD_IA`
    pub fn default_storage_class(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.default_storage_class = Some(v.to_string())
        }

        self
    }

    /// Set server_side_encryption for this backend.
    ///
    /// Available values: `AES256`, `aws:kms`.
    ///
    /// # Note
    ///
    /// This function is the low-level setting for SSE related features.
    ///
    /// SSE related options should be set carefully to make them works.
    /// Please use `server_side_encryption_with_*` helpers if even possible.
    pub fn server_side_encryption(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.server_side_encryption = Some(v.to_string())
        }

        self
    }

    /// Set server_side_encryption_aws_kms_key_id for this backend
    ///
    /// - If `server_side_encryption` set to `aws:kms`, and `server_side_encryption_aws_kms_key_id`
    ///   is not set, S3 will use aws managed kms key to encrypt data.
    /// - If `server_side_encryption` set to `aws:kms`, and `server_side_encryption_aws_kms_key_id`
    ///   is a valid kms key id, S3 will use the provided kms key to encrypt data.
    /// - If the `server_side_encryption_aws_kms_key_id` is invalid or not found, an error will be
    ///   returned.
    /// - If `server_side_encryption` is not `aws:kms`, setting `server_side_encryption_aws_kms_key_id` is a noop.
    ///
    /// # Note
    ///
    /// This function is the low-level setting for SSE related features.
    ///
    /// SSE related options should be set carefully to make them works.
    /// Please use `server_side_encryption_with_*` helpers if even possible.
    pub fn server_side_encryption_aws_kms_key_id(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.server_side_encryption_aws_kms_key_id = Some(v.to_string())
        }

        self
    }

    /// Set server_side_encryption_customer_algorithm for this backend.
    ///
    /// Available values: `AES256`.
    ///
    /// # Note
    ///
    /// This function is the low-level setting for SSE related features.
    ///
    /// SSE related options should be set carefully to make them works.
    /// Please use `server_side_encryption_with_*` helpers if even possible.
    pub fn server_side_encryption_customer_algorithm(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.server_side_encryption_customer_algorithm = Some(v.to_string())
        }

        self
    }

    /// Set server_side_encryption_customer_key for this backend.
    ///
    /// # Args
    ///
    /// `v`: base64 encoded key that matches algorithm specified in
    /// `server_side_encryption_customer_algorithm`.
    ///
    /// # Note
    ///
    /// This function is the low-level setting for SSE related features.
    ///
    /// SSE related options should be set carefully to make them works.
    /// Please use `server_side_encryption_with_*` helpers if even possible.
    pub fn server_side_encryption_customer_key(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.server_side_encryption_customer_key = Some(v.to_string())
        }

        self
    }

    /// Set server_side_encryption_customer_key_md5 for this backend.
    ///
    /// # Args
    ///
    /// `v`: MD5 digest of key specified in `server_side_encryption_customer_key`.
    ///
    /// # Note
    ///
    /// This function is the low-level setting for SSE related features.
    ///
    /// SSE related options should be set carefully to make them works.
    /// Please use `server_side_encryption_with_*` helpers if even possible.
    pub fn server_side_encryption_customer_key_md5(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.server_side_encryption_customer_key_md5 = Some(v.to_string())
        }

        self
    }

    /// Enable server side encryption with aws managed kms key
    ///
    /// As known as: SSE-KMS
    ///
    /// NOTE: This function should not be used along with other `server_side_encryption_with_` functions.
    pub fn server_side_encryption_with_aws_managed_kms_key(mut self) -> Self {
        self.config.server_side_encryption = Some("aws:kms".to_string());
        self
    }

    /// Enable server side encryption with customer managed kms key
    ///
    /// As known as: SSE-KMS
    ///
    /// NOTE: This function should not be used along with other `server_side_encryption_with_` functions.
    pub fn server_side_encryption_with_customer_managed_kms_key(
        mut self,
        aws_kms_key_id: &str,
    ) -> Self {
        self.config.server_side_encryption = Some("aws:kms".to_string());
        self.config.server_side_encryption_aws_kms_key_id = Some(aws_kms_key_id.to_string());
        self
    }

    /// Enable server side encryption with s3 managed key
    ///
    /// As known as: SSE-S3
    ///
    /// NOTE: This function should not be used along with other `server_side_encryption_with_` functions.
    pub fn server_side_encryption_with_s3_key(mut self) -> Self {
        self.config.server_side_encryption = Some("AES256".to_string());
        self
    }

    /// Enable server side encryption with customer key.
    ///
    /// As known as: SSE-C
    ///
    /// NOTE: This function should not be used along with other `server_side_encryption_with_` functions.
    pub fn server_side_encryption_with_customer_key(mut self, algorithm: &str, key: &[u8]) -> Self {
        self.config.server_side_encryption_customer_algorithm = Some(algorithm.to_string());
        self.config.server_side_encryption_customer_key = Some(BASE64_STANDARD.encode(key));
        self.config.server_side_encryption_customer_key_md5 =
            Some(BASE64_STANDARD.encode(Md5::digest(key).as_slice()));
        self
    }

    /// Set temporary credential used in AWS S3 connections
    ///
    /// # Warning
    ///
    /// session token's lifetime is short and requires users to refresh in time.
    pub fn session_token(mut self, token: &str) -> Self {
        if !token.is_empty() {
            self.config.session_token = Some(token.to_string());
        }
        self
    }

    /// Set temporary credential used in AWS S3 connections
    #[deprecated(note = "Please use `session_token` instead")]
    pub fn security_token(self, token: &str) -> Self {
        self.session_token(token)
    }

    /// Disable config load so that opendal will not load config from
    /// environment.
    ///
    /// For examples:
    ///
    /// - envs like `AWS_ACCESS_KEY_ID`
    /// - files like `~/.aws/config`
    pub fn disable_config_load(mut self) -> Self {
        self.config.disable_config_load = true;
        self
    }

    /// Disable load credential from ec2 metadata.
    ///
    /// This option is used to disable the default behavior of opendal
    /// to load credential from ec2 metadata, a.k.a, IMDSv2
    pub fn disable_ec2_metadata(mut self) -> Self {
        self.config.disable_ec2_metadata = true;
        self
    }

    /// Allow anonymous will allow opendal to send request without signing
    /// when credential is not loaded.
    pub fn allow_anonymous(mut self) -> Self {
        self.config.allow_anonymous = true;
        self
    }

    /// Enable virtual host style so that opendal will send API requests
    /// in virtual host style instead of path style.
    ///
    /// - By default, opendal will send API to `https://s3.us-east-1.amazonaws.com/bucket_name`
    /// - Enabled, opendal will send API to `https://bucket_name.s3.us-east-1.amazonaws.com`
    pub fn enable_virtual_host_style(mut self) -> Self {
        self.config.enable_virtual_host_style = true;
        self
    }

    /// Disable stat with override so that opendal will not send stat request with override queries.
    ///
    /// For example, R2 doesn't support stat with `response_content_type` query.
    pub fn disable_stat_with_override(mut self) -> Self {
        self.config.disable_stat_with_override = true;
        self
    }

    /// Adding a customized credential load for service.
    ///
    /// If customized_credential_load has been set, we will ignore all other
    /// credential load methods.
    pub fn customized_credential_load(mut self, cred: Box<dyn AwsCredentialLoad>) -> Self {
        self.customized_credential_load = Some(cred);
        self
    }

    /// Specify the http client that used by this service.
    ///
    /// # Notes
    ///
    /// This API is part of OpenDAL's Raw API. `HttpClient` could be changed
    /// during minor updates.
    pub fn http_client(mut self, client: HttpClient) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Set bucket versioning status for this backend
    pub fn enable_versioning(mut self, enabled: bool) -> Self {
        self.config.enable_versioning = enabled;

        self
    }

    /// Check if `bucket` is valid
    /// `bucket` must be not empty and if `enable_virtual_host_style` is true
    /// it couldn't contain dot(.) character
    fn is_bucket_valid(&self) -> bool {
        if self.config.bucket.is_empty() {
            return false;
        }
        // If enable virtual host style, `bucket` will reside in domain part,
        // for example `https://bucket_name.s3.us-east-1.amazonaws.com`,
        // so `bucket` with dot can't be recognized correctly for this format.
        if self.config.enable_virtual_host_style && self.config.bucket.contains('.') {
            return false;
        }
        true
    }

    /// Build endpoint with given region.
    fn build_endpoint(&self, region: &str) -> String {
        let bucket = {
            debug_assert!(self.is_bucket_valid(), "bucket must be valid");

            self.config.bucket.as_str()
        };

        let mut endpoint = match &self.config.endpoint {
            Some(endpoint) => {
                if endpoint.starts_with("http") {
                    endpoint.to_string()
                } else {
                    // Prefix https if endpoint doesn't start with scheme.
                    format!("https://{endpoint}")
                }
            }
            None => "https://s3.amazonaws.com".to_string(),
        };

        // If endpoint contains bucket name, we should trim them.
        endpoint = endpoint.replace(&format!("//{bucket}."), "//");

        // Omit default ports if specified.
        if let Ok(url) = Url::from_str(&endpoint) {
            // Remove the trailing `/` of root path.
            endpoint = url.to_string().trim_end_matches('/').to_string();
        }

        // Update with endpoint templates.
        endpoint = if let Some(template) = ENDPOINT_TEMPLATES.get(endpoint.as_str()) {
            template.replace("{region}", region)
        } else {
            // If we don't know where about this endpoint, just leave
            // them as it.
            endpoint.to_string()
        };

        // Apply virtual host style.
        if self.config.enable_virtual_host_style {
            endpoint = endpoint.replace("//", &format!("//{bucket}."))
        } else {
            write!(endpoint, "/{bucket}").expect("write into string must succeed");
        };

        endpoint
    }

    /// Set maximum batch operations of this backend.
    #[deprecated(
        since = "0.52.0",
        note = "Please use `delete_max_size` instead of `batch_max_operations`"
    )]
    pub fn batch_max_operations(mut self, batch_max_operations: usize) -> Self {
        self.config.delete_max_size = Some(batch_max_operations);

        self
    }

    /// Set maximum delete operations of this backend.
    pub fn delete_max_size(mut self, delete_max_size: usize) -> Self {
        self.config.delete_max_size = Some(delete_max_size);

        self
    }

    /// Set checksum algorithm of this backend.
    /// This is necessary when writing to AWS S3 Buckets with Object Lock enabled for example.
    ///
    /// Available options:
    /// - "crc32c"
    pub fn checksum_algorithm(mut self, checksum_algorithm: &str) -> Self {
        self.config.checksum_algorithm = Some(checksum_algorithm.to_string());

        self
    }

    /// Disable write with if match so that opendal will not send write request with if match headers.
    pub fn disable_write_with_if_match(mut self) -> Self {
        self.config.disable_write_with_if_match = true;
        self
    }

    /// Detect region of S3 bucket.
    ///
    /// # Args
    ///
    /// - endpoint: the endpoint of S3 service
    /// - bucket: the bucket of S3 service
    ///
    /// # Return
    ///
    /// - `Some(region)` means we detect the region successfully
    /// - `None` means we can't detect the region or meeting errors.
    ///
    /// # Notes
    ///
    /// We will try to detect region by the following methods.
    ///
    /// - Match endpoint with given rules to get region
    ///   - Cloudflare R2
    ///   - AWS S3
    ///   - Aliyun OSS
    /// - Send a `HEAD` request to endpoint with bucket name to get `x-amz-bucket-region`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use opendal::services::S3;
    ///
    /// # async fn example() {
    /// let region: Option<String> = S3::detect_region("https://s3.amazonaws.com", "example").await;
    /// # }
    /// ```
    ///
    /// # Reference
    ///
    /// - [Amazon S3 HeadBucket API](https://docs.aws.amazon.com/zh_cn/AmazonS3/latest/API/API_HeadBucket.html)
    pub async fn detect_region(endpoint: &str, bucket: &str) -> Option<String> {
        // Remove the possible trailing `/` in endpoint.
        let endpoint = endpoint.trim_end_matches('/');

        // Make sure the endpoint contains the scheme.
        let mut endpoint = if endpoint.starts_with("http") {
            endpoint.to_string()
        } else {
            // Prefix https if endpoint doesn't start with scheme.
            format!("https://{}", endpoint)
        };

        // Remove bucket name from endpoint.
        endpoint = endpoint.replace(&format!("//{bucket}."), "//");
        let url = format!("{endpoint}/{bucket}");

        debug!("detect region with url: {url}");

        // Try to detect region by endpoint.

        // If this bucket is R2, we can return auto directly.
        //
        // Reference: <https://developers.cloudflare.com/r2/api/s3/api/>
        if endpoint.ends_with("r2.cloudflarestorage.com") {
            return Some("auto".to_string());
        }

        // If this bucket is AWS, we can try to match the endpoint.
        if let Some(v) = endpoint.strip_prefix("https://s3.") {
            if let Some(region) = v.strip_suffix(".amazonaws.com") {
                return Some(region.to_string());
            }
        }

        // If this bucket is OSS, we can try to match the endpoint.
        //
        // - `oss-ap-southeast-1.aliyuncs.com` => `oss-ap-southeast-1`
        // - `oss-cn-hangzhou-internal.aliyuncs.com` => `oss-cn-hangzhou`
        if let Some(v) = endpoint.strip_prefix("https://") {
            if let Some(region) = v.strip_suffix(".aliyuncs.com") {
                return Some(region.to_string());
            }

            if let Some(region) = v.strip_suffix("-internal.aliyuncs.com") {
                return Some(region.to_string());
            }
        }

        // Try to detect region by HeadBucket.
        let req = http::Request::head(&url).body(Buffer::new()).ok()?;

        let client = HttpClient::new().ok()?;
        let res = client
            .send(req)
            .await
            .map_err(|err| warn!("detect region failed for: {err:?}"))
            .ok()?;

        debug!(
            "auto detect region got response: status {:?}, header: {:?}",
            res.status(),
            res.headers()
        );

        // Get region from response header no matter status code.
        if let Some(header) = res.headers().get("x-amz-bucket-region") {
            if let Ok(regin) = header.to_str() {
                return Some(regin.to_string());
            }
        }

        // Status code is 403 or 200 means we already visit the correct
        // region, we can use the default region directly.
        if res.status() == StatusCode::FORBIDDEN || res.status() == StatusCode::OK {
            return Some("us-east-1".to_string());
        }

        None
    }
}

impl Builder for S3Builder {
    const SCHEME: Scheme = Scheme::S3;
    type Config = S3Config;

    fn build(mut self) -> Result<impl Access> {
        debug!("backend build started: {:?}", &self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", &root);

        // Handle bucket name.
        let bucket = if self.is_bucket_valid() {
            Ok(&self.config.bucket)
        } else {
            Err(
                Error::new(ErrorKind::ConfigInvalid, "The bucket is misconfigured")
                    .with_context("service", Scheme::S3),
            )
        }?;
        debug!("backend use bucket {}", &bucket);

        let default_storage_class = match &self.config.default_storage_class {
            None => None,
            Some(v) => Some(
                build_header_value(v).map_err(|err| err.with_context("key", "storage_class"))?,
            ),
        };

        let server_side_encryption = match &self.config.server_side_encryption {
            None => None,
            Some(v) => Some(
                build_header_value(v)
                    .map_err(|err| err.with_context("key", "server_side_encryption"))?,
            ),
        };

        let server_side_encryption_aws_kms_key_id =
            match &self.config.server_side_encryption_aws_kms_key_id {
                None => None,
                Some(v) => Some(build_header_value(v).map_err(|err| {
                    err.with_context("key", "server_side_encryption_aws_kms_key_id")
                })?),
            };

        let server_side_encryption_customer_algorithm =
            match &self.config.server_side_encryption_customer_algorithm {
                None => None,
                Some(v) => Some(build_header_value(v).map_err(|err| {
                    err.with_context("key", "server_side_encryption_customer_algorithm")
                })?),
            };

        let server_side_encryption_customer_key =
            match &self.config.server_side_encryption_customer_key {
                None => None,
                Some(v) => Some(build_header_value(v).map_err(|err| {
                    err.with_context("key", "server_side_encryption_customer_key")
                })?),
            };

        let server_side_encryption_customer_key_md5 =
            match &self.config.server_side_encryption_customer_key_md5 {
                None => None,
                Some(v) => Some(build_header_value(v).map_err(|err| {
                    err.with_context("key", "server_side_encryption_customer_key_md5")
                })?),
            };

        let checksum_algorithm = match self.config.checksum_algorithm.as_deref() {
            Some("crc32c") => Some(ChecksumAlgorithm::Crc32c),
            None => None,
            v => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    format!("{:?} is not a supported checksum_algorithm.", v),
                ))
            }
        };

        // This is our current config.
        let mut cfg = AwsConfig::default();
        if !self.config.disable_config_load {
            #[cfg(not(target_arch = "wasm32"))]
            {
                cfg = cfg.from_profile();
                cfg = cfg.from_env();
            }
        }

        if let Some(ref v) = self.config.region {
            cfg.region = Some(v.to_string());
        }

        if cfg.region.is_none() {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "region is missing. Please find it by S3::detect_region() or set them in env.",
            )
            .with_operation("Builder::build")
            .with_context("service", Scheme::S3));
        }

        let region = cfg.region.to_owned().unwrap();
        debug!("backend use region: {region}");

        // Retain the user's endpoint if it exists; otherwise, try loading it from the environment.
        self.config.endpoint = self.config.endpoint.or_else(|| cfg.endpoint_url.clone());

        // Building endpoint.
        let endpoint = self.build_endpoint(&region);
        debug!("backend use endpoint: {endpoint}");

        // Setting all value from user input if available.
        if let Some(v) = self.config.access_key_id {
            cfg.access_key_id = Some(v)
        }
        if let Some(v) = self.config.secret_access_key {
            cfg.secret_access_key = Some(v)
        }
        if let Some(v) = self.config.session_token {
            cfg.session_token = Some(v)
        }

        let client = if let Some(client) = self.http_client {
            client
        } else {
            HttpClient::new().map_err(|err| {
                err.with_operation("Builder::build")
                    .with_context("service", Scheme::S3)
            })?
        };

        let mut loader: Option<Box<dyn AwsCredentialLoad>> = None;
        // If customized_credential_load is set, we will use it.
        if let Some(v) = self.customized_credential_load {
            loader = Some(v);
        }

        // If role_arn is set, we must use AssumeRoleLoad.
        if let Some(role_arn) = self.config.role_arn {
            // use current env as source credential loader.
            let default_loader =
                AwsDefaultLoader::new(GLOBAL_REQWEST_CLIENT.clone().clone(), cfg.clone());

            // Build the config for assume role.
            let mut assume_role_cfg = AwsConfig {
                region: Some(region.clone()),
                role_arn: Some(role_arn),
                external_id: self.config.external_id.clone(),
                sts_regional_endpoints: "regional".to_string(),
                ..Default::default()
            };

            // override default role_session_name if set
            if let Some(name) = self.config.role_session_name {
                assume_role_cfg.role_session_name = name;
            }

            let assume_role_loader = AwsAssumeRoleLoader::new(
                GLOBAL_REQWEST_CLIENT.clone().clone(),
                assume_role_cfg,
                Box::new(default_loader),
            )
            .map_err(|err| {
                Error::new(
                    ErrorKind::ConfigInvalid,
                    "The assume_role_loader is misconfigured",
                )
                .with_context("service", Scheme::S3)
                .set_source(err)
            })?;
            loader = Some(Box::new(assume_role_loader));
        }
        // If loader is not set, we will use default loader.
        let loader = match loader {
            Some(v) => v,
            None => {
                let mut default_loader =
                    AwsDefaultLoader::new(GLOBAL_REQWEST_CLIENT.clone().clone(), cfg);
                if self.config.disable_ec2_metadata {
                    default_loader = default_loader.with_disable_ec2_metadata();
                }

                Box::new(default_loader)
            }
        };

        let signer = AwsV4Signer::new("s3", &region);

        let delete_max_size = self
            .config
            .delete_max_size
            .unwrap_or(DEFAULT_BATCH_MAX_OPERATIONS);

        Ok(S3Backend {
            core: Arc::new(S3Core {
                bucket: bucket.to_string(),
                endpoint,
                root,
                server_side_encryption,
                server_side_encryption_aws_kms_key_id,
                server_side_encryption_customer_algorithm,
                server_side_encryption_customer_key,
                server_side_encryption_customer_key_md5,
                default_storage_class,
                allow_anonymous: self.config.allow_anonymous,
                disable_stat_with_override: self.config.disable_stat_with_override,
                enable_versioning: self.config.enable_versioning,
                signer,
                loader,
                credential_loaded: AtomicBool::new(false),
                client,
                checksum_algorithm,
                delete_max_size,
                disable_write_with_if_match: self.config.disable_write_with_if_match,
            }),
        })
    }
}

/// Backend for s3 services.
#[derive(Debug, Clone)]
pub struct S3Backend {
    core: Arc<S3Core>,
}

impl Access for S3Backend {
    type Reader = HttpBody;
    type Writer = S3Writers;
    type Lister = S3Listers;
    type Deleter = oio::BatchDeleter<S3Deleter>;
    type BlockingReader = ();
    type BlockingWriter = ();
    type BlockingLister = ();
    type BlockingDeleter = ();

    fn info(&self) -> Arc<AccessorInfo> {
        let mut am = AccessorInfo::default();
        am.set_scheme(Scheme::S3)
            .set_root(&self.core.root)
            .set_name(&self.core.bucket)
            .set_native_capability(Capability {
                stat: true,
                stat_has_content_encoding: true,
                stat_with_if_match: true,
                stat_with_if_none_match: true,
                stat_with_if_modified_since: true,
                stat_with_if_unmodified_since: true,
                stat_with_override_cache_control: !self.core.disable_stat_with_override,
                stat_with_override_content_disposition: !self.core.disable_stat_with_override,
                stat_with_override_content_type: !self.core.disable_stat_with_override,
                stat_with_version: self.core.enable_versioning,
                stat_has_cache_control: true,
                stat_has_content_length: true,
                stat_has_content_type: true,
                stat_has_content_range: true,
                stat_has_etag: true,
                stat_has_content_md5: true,
                stat_has_last_modified: true,
                stat_has_content_disposition: true,
                stat_has_user_metadata: true,
                stat_has_version: true,

                read: true,
                read_with_if_match: true,
                read_with_if_none_match: true,
                read_with_if_modified_since: true,
                read_with_if_unmodified_since: true,
                read_with_override_cache_control: true,
                read_with_override_content_disposition: true,
                read_with_override_content_type: true,
                read_with_version: self.core.enable_versioning,

                write: true,
                write_can_empty: true,
                write_can_multi: true,
                write_with_cache_control: true,
                write_with_content_type: true,
                write_with_content_encoding: true,
                write_with_if_match: !self.core.disable_write_with_if_match,
                write_with_if_not_exists: true,
                write_with_user_metadata: true,

                // The min multipart size of S3 is 5 MiB.
                //
                // ref: <https://docs.aws.amazon.com/AmazonS3/latest/userguide/qfacts.html>
                write_multi_min_size: Some(5 * 1024 * 1024),
                // The max multipart size of S3 is 5 GiB.
                //
                // ref: <https://docs.aws.amazon.com/AmazonS3/latest/userguide/qfacts.html>
                write_multi_max_size: if cfg!(target_pointer_width = "64") {
                    Some(5 * 1024 * 1024 * 1024)
                } else {
                    Some(usize::MAX)
                },

                delete: true,
                delete_max_size: Some(self.core.delete_max_size),
                delete_with_version: self.core.enable_versioning,

                copy: true,

                list: true,
                list_with_limit: true,
                list_with_start_after: true,
                list_with_recursive: true,
                list_with_versions: self.core.enable_versioning,
                list_with_deleted: self.core.enable_versioning,
                list_has_etag: true,
                list_has_content_md5: true,
                list_has_content_length: true,
                list_has_last_modified: true,

                presign: true,
                presign_stat: true,
                presign_read: true,
                presign_write: true,

                shared: true,

                ..Default::default()
            });

        am.into()
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let resp = self.core.s3_head_object(path, args).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let headers = resp.headers();
                let mut meta = parse_into_metadata(path, headers)?;

                let user_meta = parse_prefixed_headers(headers, X_AMZ_META_PREFIX);
                if !user_meta.is_empty() {
                    meta.with_user_metadata(user_meta);
                }

                if let Some(v) = parse_header_to_str(headers, X_AMZ_VERSION_ID)? {
                    meta.set_version(v);
                }

                Ok(RpStat::new(meta))
            }
            _ => Err(parse_error(resp)),
        }
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let resp = self.core.s3_get_object(path, args.range(), &args).await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                Ok((RpRead::default(), resp.into_body()))
            }
            _ => {
                let (part, mut body) = resp.into_parts();
                let buf = body.to_buffer().await?;
                Err(parse_error(Response::from_parts(part, buf)))
            }
        }
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let concurrent = args.concurrent();
        let executor = args.executor().cloned();
        let writer = S3Writer::new(self.core.clone(), path, args);

        let w = oio::MultipartWriter::new(writer, executor, concurrent);

        Ok((RpWrite::default(), w))
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        Ok((
            RpDelete::default(),
            oio::BatchDeleter::new(S3Deleter::new(self.core.clone())),
        ))
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        let l = if args.versions() || args.deleted() {
            TwoWays::Two(PageLister::new(S3ObjectVersionsLister::new(
                self.core.clone(),
                path,
                args,
            )))
        } else {
            TwoWays::One(PageLister::new(S3Lister::new(
                self.core.clone(),
                path,
                args,
            )))
        };

        Ok((RpList::default(), l))
    }

    async fn copy(&self, from: &str, to: &str, _args: OpCopy) -> Result<RpCopy> {
        let resp = self.core.s3_copy_object(from, to).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(RpCopy::default()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        let (expire, op) = args.into_parts();

        // We will not send this request out, just for signing.
        let mut req = match op {
            PresignOperation::Stat(v) => self.core.s3_head_object_request(path, v)?,
            PresignOperation::Read(v) => {
                self.core
                    .s3_get_object_request(path, BytesRange::default(), &v)?
            }
            PresignOperation::Write(_) => {
                self.core
                    .s3_put_object_request(path, None, &OpWrite::default(), Buffer::new())?
            }
        };

        self.core.sign_query(&mut req, expire).await?;

        // We don't need this request anymore, consume it directly.
        let (parts, _) = req.into_parts();

        Ok(RpPresign::new(PresignedRequest::new(
            parts.method,
            parts.uri,
            parts.headers,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_bucket() {
        let bucket_cases = vec![
            ("", false, false),
            ("test", false, true),
            ("test.xyz", false, true),
            ("", true, false),
            ("test", true, true),
            ("test.xyz", true, false),
        ];

        for (bucket, enable_virtual_host_style, expected) in bucket_cases {
            let mut b = S3Builder::default();
            b = b.bucket(bucket);
            if enable_virtual_host_style {
                b = b.enable_virtual_host_style();
            }
            assert_eq!(b.is_bucket_valid(), expected)
        }
    }

    #[test]
    fn test_build_endpoint() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        let endpoint_cases = vec![
            Some("s3.amazonaws.com"),
            Some("https://s3.amazonaws.com"),
            Some("https://s3.us-east-2.amazonaws.com"),
            None,
        ];

        for endpoint in &endpoint_cases {
            let mut b = S3Builder::default().bucket("test");
            if let Some(endpoint) = endpoint {
                b = b.endpoint(endpoint);
            }

            let endpoint = b.build_endpoint("us-east-2");
            assert_eq!(endpoint, "https://s3.us-east-2.amazonaws.com/test");
        }

        for endpoint in &endpoint_cases {
            let mut b = S3Builder::default()
                .bucket("test")
                .enable_virtual_host_style();
            if let Some(endpoint) = endpoint {
                b = b.endpoint(endpoint);
            }

            let endpoint = b.build_endpoint("us-east-2");
            assert_eq!(endpoint, "https://test.s3.us-east-2.amazonaws.com");
        }
    }

    #[tokio::test]
    async fn test_detect_region() {
        let cases = vec![
            (
                "aws s3 without region in endpoint",
                "https://s3.amazonaws.com",
                "example",
                Some("us-east-1"),
            ),
            (
                "aws s3 with region in endpoint",
                "https://s3.us-east-1.amazonaws.com",
                "example",
                Some("us-east-1"),
            ),
            (
                "oss with public endpoint",
                "https://oss-ap-southeast-1.aliyuncs.com",
                "example",
                Some("oss-ap-southeast-1"),
            ),
            (
                "oss with internal endpoint",
                "https://oss-cn-hangzhou-internal.aliyuncs.com",
                "example",
                Some("oss-cn-hangzhou-internal"),
            ),
            (
                "r2",
                "https://abc.xxxxx.r2.cloudflarestorage.com",
                "example",
                Some("auto"),
            ),
            (
                "invalid service",
                "https://opendal.apache.org",
                "example",
                None,
            ),
        ];

        for (name, endpoint, bucket, expected) in cases {
            let region = S3Builder::detect_region(endpoint, bucket).await;
            assert_eq!(region.as_deref(), expected, "{}", name);
        }
    }
}
