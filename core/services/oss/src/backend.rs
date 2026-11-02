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
use std::sync::Arc;

use http::StatusCode;
use http::Uri;
use log::debug;
use reqsign_aliyun_oss::AssumeRoleCredentialProvider;
use reqsign_aliyun_oss::AssumeRoleWithOidcCredentialProvider;
use reqsign_aliyun_oss::EcsRamRoleCredentialProvider;
use reqsign_aliyun_oss::EnvCredentialProvider;
use reqsign_aliyun_oss::RequestSigner;
use reqsign_aliyun_oss::StaticCredentialProvider;
use reqsign_core::Context;
use reqsign_core::Env as _;
use reqsign_core::OsEnv;
use reqsign_core::ProvideCredentialChain;
use reqsign_core::Signer;
use reqsign_core::StaticEnv;
use reqsign_file_read_tokio::TokioFileRead;

use super::OSS_SCHEME;
use super::config::OssConfig;
use super::core::parse_error;
use super::core::*;
use super::deleter::OssDeleter;
use super::lister::OssLister;
use super::lister::OssListers;
use super::lister::OssObjectVersionsLister;
use super::reader::*;
use super::writer::OssWriter;
use super::writer::OssWriters;
use opendal_core::raw::*;
use opendal_core::*;

const DEFAULT_BATCH_MAX_OPERATIONS: usize = 1000;

/// Aliyun Object Storage Service (OSS) support
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct OssBuilder {
    pub(super) config: OssConfig,
}

impl Debug for OssBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OssBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl OssBuilder {
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
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            // Trim trailing `/` so that we can accept `http://127.0.0.1:9000/`
            self.config.endpoint = Some(endpoint.trim_end_matches('/').to_string())
        }

        self
    }

    /// Set addressing style for the endpoint.
    ///
    /// Available values: `virtual`, `cname`, `path`.
    ///
    /// - `virtual`: Use virtual addressing style, i.e. `http://bucket.oss-<region>.aliyuncs.com/object`
    /// - `cname`: Use cname addressing style, i.e. `http://mydomain.com/object` with mydomain.com bound to your bucket.
    /// - `path`: Use path addressing style. i.e. `http://oss-<region>.aliyuncs.com/bucket/object`
    ///
    /// - If not set, default value is `virtual`.
    pub fn addressing_style(mut self, addressing_style: &str) -> Self {
        self.config.addressing_style = Some(addressing_style.to_string());

        self
    }

    /// Deprecated: OSS versioning capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "OSS versioning capability is enabled by default and this option is no longer needed."
    )]
    pub fn enable_versioning(self, _enabled: bool) -> Self {
        self
    }

    /// Set an endpoint for generating presigned urls.
    ///
    /// You can offer a public endpoint like <https://oss-cn-beijing.aliyuncs.com> to return a presinged url for
    /// public accessors, along with an internal endpoint like <https://oss-cn-beijing-internal.aliyuncs.com>
    /// to access objects in a faster path.
    ///
    /// - If presign_endpoint is set, we will use presign_endpoint on generating presigned urls.
    /// - if not, we will use endpoint as default.
    pub fn presign_endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            // Trim trailing `/` so that we can accept `http://127.0.0.1:9000/`
            self.config.presign_endpoint = Some(endpoint.trim_end_matches('/').to_string())
        }

        self
    }

    /// Set addressing style for presign endpoint.
    ///
    /// Similar to setting addressing style for endpoint.
    ///
    /// - If both presign_endpoint and presign_addressing_style are not set, they are the same as endpoint's configurations.
    ///
    /// - If presign_endpoint is set, but presign_addressing_style is not set, default value is `virtual`.
    pub fn presign_addressing_style(mut self, addressing_style: &str) -> Self {
        self.config.presign_addressing_style = Some(addressing_style.to_string());

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

    /// Set access_key_secret of this backend.
    ///
    /// - If access_key_secret is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn access_key_secret(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.access_key_secret = Some(v.to_string())
        }

        self
    }

    /// Set security_token for this backend.
    ///
    /// - If security_token is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn security_token(mut self, security_token: &str) -> Self {
        if !security_token.is_empty() {
            self.config.security_token = Some(security_token.to_string())
        }

        self
    }

    /// preprocess the endpoint option
    fn parse_endpoint(
        &self,
        endpoint: &Option<String>,
        bucket: &str,
        addressing_style: AddressingStyle,
    ) -> Result<(String, String)> {
        let (endpoint, host) = match endpoint.clone() {
            Some(ep) => {
                let uri = ep.parse::<Uri>().map_err(|err| {
                    Error::new(ErrorKind::ConfigInvalid, "endpoint is invalid")
                        .with_context("service", OSS_SCHEME)
                        .with_context("endpoint", &ep)
                        .set_source(err)
                })?;
                let host = uri.host().ok_or_else(|| {
                    Error::new(ErrorKind::ConfigInvalid, "endpoint host is empty")
                        .with_context("service", OSS_SCHEME)
                        .with_context("endpoint", &ep)
                })?;
                let full_host = match addressing_style {
                    AddressingStyle::Virtual => {
                        if let Some(port) = uri.port_u16() {
                            format!("{bucket}.{host}:{port}")
                        } else {
                            format!("{bucket}.{host}")
                        }
                    }
                    AddressingStyle::Cname | AddressingStyle::Path => {
                        if let Some(port) = uri.port_u16() {
                            format!("{host}:{port}")
                        } else {
                            host.to_string()
                        }
                    }
                };
                if let Some(port) = uri.port_u16() {
                    format!("{bucket}.{host}:{port}")
                } else {
                    format!("{bucket}.{host}")
                };
                let endpoint = match uri.scheme_str() {
                    Some(scheme_str) => match scheme_str {
                        "http" | "https" => format!("{scheme_str}://{full_host}"),
                        _ => {
                            return Err(Error::new(
                                ErrorKind::ConfigInvalid,
                                "endpoint protocol is invalid",
                            )
                            .with_context("service", OSS_SCHEME));
                        }
                    },
                    None => format!("https://{full_host}"),
                };
                let endpoint = match addressing_style {
                    AddressingStyle::Path => format!("{}/{}", endpoint, bucket),
                    AddressingStyle::Cname | AddressingStyle::Virtual => endpoint,
                };
                (endpoint, full_host)
            }
            None => {
                return Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                    .with_context("service", OSS_SCHEME));
            }
        };
        Ok((endpoint, host))
    }

    /// Set server_side_encryption for this backend.
    ///
    /// Available values: `AES256`, `KMS`.
    ///
    /// Reference: <https://www.alibabacloud.com/help/en/object-storage-service/latest/server-side-encryption-5>
    /// Brief explanation:
    /// There are two server-side encryption methods available:
    /// SSE-AES256:
    ///     1. Configure the bucket encryption mode as OSS-managed and specify the encryption algorithm as AES256.
    ///     2. Include the `x-oss-server-side-encryption` parameter in the request and set its value to AES256.
    /// SSE-KMS:
    ///     1. To use this service, you need to first enable KMS.
    ///     2. Configure the bucket encryption mode as KMS, and specify the specific CMK ID for BYOK (Bring Your Own Key)
    ///        or not specify the specific CMK ID for OSS-managed KMS key.
    ///     3. Include the `x-oss-server-side-encryption` parameter in the request and set its value to KMS.
    ///     4. If a specific CMK ID is specified, include the `x-oss-server-side-encryption-key-id` parameter in the request, and set its value to the specified CMK ID.
    pub fn server_side_encryption(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.server_side_encryption = Some(v.to_string())
        }
        self
    }

    /// Set server_side_encryption_key_id for this backend.
    ///
    /// # Notes
    ///
    /// This option only takes effect when server_side_encryption equals to KMS.
    pub fn server_side_encryption_key_id(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.server_side_encryption_key_id = Some(v.to_string())
        }
        self
    }

    /// Deprecated: OSS delete batch capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "OSS delete batch capability is enabled by default and this option is no longer needed."
    )]
    pub fn batch_max_operations(self, _delete_max_size: usize) -> Self {
        self
    }

    /// Deprecated: OSS delete batch capability is enabled by default.
    #[deprecated(
        since = "0.57.0",
        note = "OSS delete batch capability is enabled by default and this option is no longer needed."
    )]
    pub fn delete_max_size(self, _delete_max_size: usize) -> Self {
        self
    }

    /// Skip signature will skip loading credentials and signing requests.
    pub fn skip_signature(mut self) -> Self {
        self.config.skip_signature = true;
        self
    }

    /// Allow anonymous will allow opendal to send request without signing
    /// when credential is not loaded.
    #[deprecated(
        since = "0.57.0",
        note = "Please use `skip_signature` instead of `allow_anonymous`"
    )]
    pub fn allow_anonymous(self) -> Self {
        self.skip_signature()
    }

    /// Set role_arn for this backend.
    ///
    /// If `role_arn` is set, we will use already known config as source
    /// credential to assume role with `role_arn`.
    pub fn role_arn(mut self, role_arn: &str) -> Self {
        if !role_arn.is_empty() {
            self.config.role_arn = Some(role_arn.to_string())
        }

        self
    }

    /// Set role_session_name for this backend.
    pub fn role_session_name(mut self, role_session_name: &str) -> Self {
        if !role_session_name.is_empty() {
            self.config.role_session_name = Some(role_session_name.to_string())
        }

        self
    }

    /// Set oidc_provider_arn for this backend.
    pub fn oidc_provider_arn(mut self, oidc_provider_arn: &str) -> Self {
        if !oidc_provider_arn.is_empty() {
            self.config.oidc_provider_arn = Some(oidc_provider_arn.to_string())
        }

        self
    }

    /// Set oidc_token_file for this backend.
    pub fn oidc_token_file(mut self, oidc_token_file: &str) -> Self {
        if !oidc_token_file.is_empty() {
            self.config.oidc_token_file = Some(oidc_token_file.to_string())
        }

        self
    }

    /// Set sts_endpoint for this backend.
    pub fn sts_endpoint(mut self, sts_endpoint: &str) -> Self {
        if !sts_endpoint.is_empty() {
            self.config.sts_endpoint = Some(sts_endpoint.to_string())
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
}

enum AddressingStyle {
    Path,
    Cname,
    Virtual,
}

impl TryFrom<&Option<String>> for AddressingStyle {
    type Error = Error;

    fn try_from(value: &Option<String>) -> Result<Self> {
        match value.as_deref() {
            None | Some("virtual") => Ok(AddressingStyle::Virtual),
            Some("path") => Ok(AddressingStyle::Path),
            Some("cname") => Ok(AddressingStyle::Cname),
            Some(v) => Err(Error::new(
                ErrorKind::ConfigInvalid,
                "Invalid addressing style, available: `virtual`, `path`, `cname`",
            )
            .with_context("service", OSS_SCHEME)
            .with_context("addressing_style", v)),
        }
    }
}

impl Builder for OssBuilder {
    type Config = OssConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        #[allow(deprecated)]
        let skip_signature = self.config.skip_signature || self.config.allow_anonymous;

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", root);

        // Handle endpoint, region and bucket name.
        let bucket = match self.config.bucket.is_empty() {
            false => Ok(&self.config.bucket),
            true => Err(
                Error::new(ErrorKind::ConfigInvalid, "The bucket is misconfigured")
                    .with_context("service", OSS_SCHEME),
            ),
        }?;

        // Retrieve endpoint and host by parsing the endpoint option and bucket. If presign_endpoint is not
        // set, take endpoint as default presign_endpoint.
        let (endpoint, host) = self.parse_endpoint(
            &self.config.endpoint,
            bucket,
            (&self.config.addressing_style).try_into()?,
        )?;
        debug!("backend use bucket {}, endpoint: {}", bucket, endpoint);

        let presign_endpoint = if self.config.presign_endpoint.is_some() {
            self.parse_endpoint(
                &self.config.presign_endpoint,
                bucket,
                (&self.config.presign_addressing_style).try_into()?,
            )?
            .0
        } else {
            endpoint.clone()
        };
        debug!("backend use presign_endpoint: {}", presign_endpoint);

        let server_side_encryption = match &self.config.server_side_encryption {
            None => None,
            Some(v) => Some(
                build_header_value(v)
                    .map_err(|err| err.with_context("key", "server_side_encryption"))?,
            ),
        };

        let server_side_encryption_key_id = match &self.config.server_side_encryption_key_id {
            None => None,
            Some(v) => Some(
                build_header_value(v)
                    .map_err(|err| err.with_context("key", "server_side_encryption_key_id"))?,
            ),
        };

        // NOTE: `AssumeRoleWithOidcCredentialProvider` still reads `role_arn`, `oidc_provider_arn`
        // and `oidc_token_file` from `Context` environment variables at runtime. Until reqsign
        // exposes typed builder APIs for all of them, we overlay config values into a `StaticEnv`
        // snapshot here.
        let os_env = OsEnv;
        let mut envs = os_env.vars();

        if let Some(v) = &self.config.role_arn {
            envs.insert("ALIBABA_CLOUD_ROLE_ARN".to_string(), v.clone());
        }
        if let Some(v) = &self.config.oidc_provider_arn {
            envs.insert("ALIBABA_CLOUD_OIDC_PROVIDER_ARN".to_string(), v.clone());
        }
        if let Some(v) = &self.config.oidc_token_file {
            envs.insert("ALIBABA_CLOUD_OIDC_TOKEN_FILE".to_string(), v.clone());
        }

        let mut assume_role = AssumeRoleWithOidcCredentialProvider::new();

        if let Some(sts_endpoint) = &self.config.sts_endpoint {
            if sts_endpoint.starts_with("http://") || sts_endpoint.starts_with("https://") {
                assume_role = assume_role.with_sts_endpoint(sts_endpoint.clone());
            } else {
                envs.insert(
                    "ALIBABA_CLOUD_STS_ENDPOINT".to_string(),
                    sts_endpoint.clone(),
                );
            }
        }

        if let Some(role_session_name) = &self.config.role_session_name {
            assume_role = assume_role.with_role_session_name(role_session_name.clone());
        }

        let ctx = Context::new()
            .with_file_read(TokioFileRead)
            .with_env(StaticEnv {
                home_dir: os_env.home_dir(),
                envs,
            });

        let static_provider = if let (Some(ak), Some(sk)) =
            (&self.config.access_key_id, &self.config.access_key_secret)
        {
            Some(if let Some(token) = self.config.security_token.as_deref() {
                StaticCredentialProvider::new(ak, sk).with_security_token(token)
            } else {
                StaticCredentialProvider::new(ak, sk)
            })
        } else {
            None
        };

        let mut provider = ProvideCredentialChain::new();
        if let Some(static_provider) = static_provider {
            provider = provider.push(static_provider);
        }
        let mut provider = provider
            .push(EnvCredentialProvider::new())
            .push(assume_role)
            .push(EcsRamRoleCredentialProvider::new());

        if let Some(role_arn) = &self.config.role_arn {
            let mut assume_role_with_ak = AssumeRoleCredentialProvider::new()
                .with_base_provider(provider)
                .with_role_arn(role_arn.clone());

            if let Some(role_session_name) = &self.config.role_session_name {
                assume_role_with_ak =
                    assume_role_with_ak.with_role_session_name(role_session_name.clone());
            }
            if let Some(external_id) = &self.config.external_id {
                assume_role_with_ak = assume_role_with_ak.with_external_id(external_id.clone());
            }
            if let Some(sts_endpoint) = &self.config.sts_endpoint {
                assume_role_with_ak = assume_role_with_ak.with_sts_endpoint(sts_endpoint.clone());
            }

            provider = ProvideCredentialChain::new().push(assume_role_with_ak);
        }

        let sign_ctx = ctx;
        let request_signer = RequestSigner::new(bucket);
        let signer = Signer::new(sign_ctx.clone(), provider, request_signer);

        let info = ServiceInfo::new(OSS_SCHEME, &root, bucket);
        let capability = Capability {
            stat: true,
            stat_with_if_match: true,
            stat_with_if_none_match: true,
            stat_with_version: true,

            read: true,
            read_with_suffix: true,

            read_with_if_match: true,
            read_with_if_none_match: true,
            read_with_version: true,
            read_with_if_modified_since: true,
            read_with_if_unmodified_since: true,

            write: true,
            write_can_empty: true,
            write_can_append: true,
            write_can_multi: true,
            write_with_cache_control: true,
            write_with_content_type: true,
            write_with_content_disposition: true,
            write_with_content_encoding: true,
            write_with_if_not_exists: true,

            // The min multipart size of OSS is 100 KiB.
            //
            // ref: <https://www.alibabacloud.com/help/en/oss/user-guide/multipart-upload-12>
            write_multi_min_size: Some(100 * 1024),
            // The max multipart size of OSS is 5 GiB.
            //
            // ref: <https://www.alibabacloud.com/help/en/oss/user-guide/multipart-upload-12>
            write_multi_max_size: if cfg!(target_pointer_width = "64") {
                Some(5 * 1024 * 1024 * 1024)
            } else {
                Some(usize::MAX)
            },
            write_with_user_metadata: true,

            delete: true,
            delete_with_version: true,
            delete_max_size: Some(DEFAULT_BATCH_MAX_OPERATIONS),

            copy: true,

            list: true,
            list_with_limit: true,
            list_with_start_after: true,
            list_with_recursive: true,
            list_with_versions: true,
            list_with_deleted: true,

            presign: true,
            presign_stat: true,
            presign_read: true,
            presign_write: true,

            shared: true,

            ..Default::default()
        };

        Ok(OssBackend {
            core: Arc::new(OssCore {
                info,
                capability,
                root,
                bucket: bucket.to_owned(),
                endpoint,
                host,
                presign_endpoint,
                skip_signature,
                signer,
                sign_ctx,
                server_side_encryption,
                server_side_encryption_key_id,
            }),
        })
    }
}

#[derive(Debug, Clone)]
/// Aliyun Object Storage Service backend
pub struct OssBackend {
    pub(crate) core: Arc<OssCore>,
}

impl Service for OssBackend {
    type Reader = oio::StreamReader<OssReader>;
    type Writer = OssWriters;
    type Lister = OssListers;
    type Deleter = oio::BatchDeleter<OssDeleter>;
    type Copier = oio::OneShotCopier;
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info.clone()
    }

    fn capability(&self) -> Capability {
        self.core.capability
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let resp = self.core.oss_head_object(ctx, path, &args).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let headers = resp.headers();
                let mut meta = self
                    .core
                    .parse_metadata(path, resp.headers())?
                    .into_builder();

                if let Some(v) = parse_header_to_str(headers, constants::X_OSS_VERSION_ID)? {
                    meta.version(v);
                }

                Ok(RpStat::new(meta.build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("HeadObject"))
                    .with_caller_condition(args.is_conditional()),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<OssReader> = {
            Ok(oio::StreamReader::new(OssReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: OssWriters = {
            let writer = OssWriter::new(self.core.clone(), ctx.clone(), path, args.clone());

            let w = if args.append() {
                OssWriters::Two(oio::AppendWriter::new(writer))
            } else {
                OssWriters::One(oio::MultipartWriter::new(
                    ctx.executor().clone(),
                    writer,
                    args.concurrent(),
                ))
            };

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::BatchDeleter<OssDeleter> = {
            Ok(oio::BatchDeleter::new(
                OssDeleter::new(self.core.clone(), ctx.clone()),
                self.core.capability.delete_max_size,
            ))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: OssListers = {
            let l = if args.versions() || args.deleted() {
                TwoWays::Two(oio::PageLister::new(OssObjectVersionsLister::new(
                    self.core.clone(),
                    ctx.clone(),
                    path,
                    args,
                )))
            } else {
                TwoWays::One(oio::PageLister::new(OssLister::new(
                    self.core.clone(),
                    ctx.clone(),
                    path,
                    args.recursive(),
                    args.limit(),
                    args.start_after(),
                )))
            };

            Ok(l)
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();
        Ok(oio::OneShotCopier::new(async move {
            let source_size = match args.source_content_length_hint() {
                Some(size) => size,
                None => {
                    let stat_args: OpStat = options::StatOptions {
                        version: args.source_version().map(str::to_owned),
                        ..Default::default()
                    }
                    .into();
                    let resp = core.oss_head_object(&ctx, &from, &stat_args).await?;
                    match resp.status() {
                        StatusCode::OK => {
                            parse_into_metadata(&from, resp.headers())?.content_length()
                        }
                        _ => {
                            return Err(parse_error(
                                ErrorContext::new(ServiceOperation("HeadObject")),
                                resp,
                            ));
                        }
                    }
                }
            };
            let resp = core.oss_copy_object(&ctx, &from, &to).await?;
            let status = resp.status();

            match status {
                StatusCode::OK => Ok(MetadataBuilder::file(source_size).build()),
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyObject")),
                    resp,
                )),
            }
        }))
    }

    async fn rename(
        &self,
        _ctx: &OperationContext,
        _from: &str,
        _to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        // We will not send this request out, just for signing.
        let req = match args.operation() {
            PresignOperation::Stat(v) => self.core.oss_head_object_request(path, true, v),
            PresignOperation::Read(range, v) => {
                self.core.oss_get_object_request(path, true, *range, v)
            }
            PresignOperation::Write(v) => {
                self.core
                    .oss_put_object_request(path, None, v, Buffer::new(), true)
            }
            PresignOperation::Delete(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            )),
            _ => Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            )),
        };
        let req = req?;
        let req = self.core.sign_query(ctx, req, args.expire()).await?;

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
    use std::collections::HashMap;
    use std::sync::Mutex;

    use bytes::Bytes;
    use reqsign_core::{FileRead, HttpSend};

    use super::*;

    #[derive(Debug)]
    struct OidcTokenFile;

    impl FileRead for OidcTokenFile {
        async fn file_read(&self, path: &str) -> reqsign_core::Result<Vec<u8>> {
            assert_eq!(path, "/test/oidc-token");
            Ok(b"test-oidc-token".to_vec())
        }
    }

    #[derive(Clone, Debug, Default)]
    struct CredentialHttpSend {
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl HttpSend for CredentialHttpSend {
        async fn http_send(
            &self,
            req: http::Request<Bytes>,
        ) -> reqsign_core::Result<http::Response<Bytes>> {
            let host = req.uri().host().unwrap();
            let path = req.uri().path();
            self.requests.lock().unwrap().push(format!("{host}{path}"));
            let body = match (host, path) {
                ("sts.aliyuncs.com", "/") => {
                    assert!(
                        req.uri()
                            .query()
                            .unwrap()
                            .contains("Action=AssumeRoleWithOIDC")
                    );
                    r#"{"Credentials":{"AccessKeyId":"oidc-ak","AccessKeySecret":"oidc-secret","SecurityToken":"oidc-token","Expiration":"2099-01-01T00:00:00Z"}}"#
                }
                ("100.100.100.200", "/latest/api/token") => "metadata-token",
                ("100.100.100.200", "/latest/meta-data/ram/security-credentials/") => "node-role",
                ("100.100.100.200", "/latest/meta-data/ram/security-credentials/node-role") => {
                    r#"{"Code":"Success","AccessKeyId":"ecs-ak","AccessKeySecret":"ecs-secret","SecurityToken":"ecs-token","Expiration":"2099-01-01T00:00:00Z"}"#
                }
                _ => panic!("unexpected credential request: {}", req.uri()),
            };
            Ok(http::Response::new(Bytes::from_static(body.as_bytes())))
        }
    }

    #[tokio::test]
    async fn test_credential_precedence() {
        for (static_keys, env_keys, oidc, expected_key, expected_token) in [
            (true, true, true, "static-ak", "static-token"),
            (false, true, true, "env-ak", "env-token"),
            (false, false, true, "oidc-ak", "oidc-token"),
            (false, false, false, "ecs-ak", "ecs-token"),
        ] {
            let mut builder = OssBuilder::default()
                .bucket("test-bucket")
                .endpoint("https://oss-cn-hangzhou.aliyuncs.com");
            if static_keys {
                builder = builder
                    .access_key_id("static-ak")
                    .access_key_secret("static-secret")
                    .security_token("static-token");
            }
            let backend = builder.build().unwrap();
            let backend = (&backend as &dyn std::any::Any)
                .downcast_ref::<OssBackend>()
                .unwrap();
            let mut envs = HashMap::new();
            if env_keys {
                envs.extend([
                    ("ALIBABA_CLOUD_ACCESS_KEY_ID".into(), "env-ak".into()),
                    (
                        "ALIBABA_CLOUD_ACCESS_KEY_SECRET".into(),
                        "env-secret".into(),
                    ),
                    ("ALIBABA_CLOUD_SECURITY_TOKEN".into(), "env-token".into()),
                ]);
            }
            if oidc {
                envs.extend([
                    (
                        "ALIBABA_CLOUD_ROLE_ARN".into(),
                        "acs:ram::123456789012:role/pod-role".into(),
                    ),
                    (
                        "ALIBABA_CLOUD_OIDC_PROVIDER_ARN".into(),
                        "acs:ram::123456789012:oidc-provider/test".into(),
                    ),
                    (
                        "ALIBABA_CLOUD_OIDC_TOKEN_FILE".into(),
                        "/test/oidc-token".into(),
                    ),
                ]);
            }
            let http = CredentialHttpSend::default();
            let ctx = Context::new()
                .with_env(StaticEnv {
                    home_dir: None,
                    envs,
                })
                .with_file_read(OidcTokenFile)
                .with_http_send(http.clone());
            let signer = backend.core.signer.clone().with_context(ctx);
            let (mut parts, _) = http::Request::builder()
                .method(http::Method::PUT)
                .uri("https://test-bucket.oss-cn-hangzhou.aliyuncs.com/manifest.json")
                .body(())
                .unwrap()
                .into_parts();
            signer.sign(&mut parts, None).await.unwrap();
            assert!(
                parts.headers[http::header::AUTHORIZATION]
                    .to_str()
                    .unwrap()
                    .starts_with(&format!("OSS {expected_key}:"))
            );
            assert_eq!(parts.headers["x-oss-security-token"], expected_token);
            let expected_requests = if static_keys || env_keys {
                vec![]
            } else if oidc {
                vec!["sts.aliyuncs.com/"]
            } else {
                vec![
                    "100.100.100.200/latest/api/token",
                    "100.100.100.200/latest/meta-data/ram/security-credentials/",
                    "100.100.100.200/latest/meta-data/ram/security-credentials/node-role",
                ]
            };
            assert_eq!(*http.requests.lock().unwrap(), expected_requests);
        }
    }
}
