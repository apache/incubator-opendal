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

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use http::StatusCode;
use log::debug;
use reqsign_azure_storage::Credential;
use reqsign_azure_storage::DefaultCredentialProvider;
use reqsign_azure_storage::RequestSigner;
use reqsign_azure_storage::StaticCredentialProvider;
use reqsign_core::Context;
use reqsign_core::OsEnv;
use reqsign_core::ProvideCredentialChain;
use reqsign_core::Signer;
use reqsign_file_read_tokio::TokioFileRead;
use sha2::Digest;
use sha2::Sha256;

use super::AZBLOB_SCHEME;
use super::config::AzblobConfig;
use super::copier::AzblobCopiers;
use super::copier::new_azblob_copier;
use super::core::AzblobCore;
use super::core::ErrorContext;
use super::core::constants::AZBLOB_COPY_MAX_BLOCK_SIZE;
use super::core::constants::AZBLOB_COPY_MIN_BLOCK_SIZE;
use super::core::constants::X_MS_META_PREFIX;
use super::core::constants::X_MS_VERSION_ID;
use super::core::parse_error;
use super::deleter::AzblobDeleter;
use super::lister::AzblobLister;
use super::reader::*;
use super::writer::AzblobWriter;
use super::writer::AzblobWriters;
use opendal_core::raw::*;
use opendal_core::*;
use opendal_service_azure_common::{
    AzureStorageConfig as AzureConnectionConfig, AzureStorageService,
    azure_account_name_from_endpoint, azure_config_from_connection_string,
};

const AZBLOB_BATCH_LIMIT: usize = 256;

impl From<AzureConnectionConfig> for AzblobConfig {
    fn from(value: AzureConnectionConfig) -> Self {
        Self {
            endpoint: value.endpoint,
            account_name: value.account_name,
            account_key: value.account_key,
            sas_token: value.sas_token,
            ..Default::default()
        }
    }
}

#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct AzblobBuilder {
    pub(super) config: AzblobConfig,
    pub(super) credential_providers: Option<ProvideCredentialChain<Credential>>,
}

impl Debug for AzblobBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzblobBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AzblobBuilder {
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

    /// Set container name of this backend.
    pub fn container(mut self, container: &str) -> Self {
        self.config.container = container.to_string();

        self
    }

    /// Set endpoint of this backend
    ///
    /// Endpoint must be full uri, e.g.
    ///
    /// - Azblob: `https://accountname.blob.core.windows.net`
    /// - Azurite: `http://127.0.0.1:10000/devstoreaccount1`
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            // Trim trailing `/` so that we can accept `http://127.0.0.1:9000/`
            self.config.endpoint = Some(endpoint.trim_end_matches('/').to_string());
        }

        self
    }

    /// Set account_name of this backend.
    ///
    /// - If account_name is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn account_name(mut self, account_name: &str) -> Self {
        if !account_name.is_empty() {
            self.config.account_name = Some(account_name.to_string());
        }

        self
    }

    /// Set account_key of this backend.
    ///
    /// - If account_key is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn account_key(mut self, account_key: &str) -> Self {
        if !account_key.is_empty() {
            self.config.account_key = Some(account_key.to_string());
        }

        self
    }

    /// Set encryption_key of this backend.
    ///
    /// # Args
    ///
    /// `v`: Base64-encoded AES-256 key that decodes to exactly 32 bytes.
    /// Building an operator returns [`ErrorKind::ConfigInvalid`] if the key
    /// is not valid Base64 or has a different decoded length.
    ///
    /// # Note
    ///
    /// This function is the low-level setting for SSE related features.
    ///
    /// SSE related options should be set carefully to make them works.
    /// Please use `server_side_encryption_with_*` helpers if even possible.
    pub fn encryption_key(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.encryption_key = Some(v.to_string());
        }

        self
    }

    /// Set encryption_key_sha256 of this backend.
    ///
    /// # Args
    ///
    /// `v`: Base64-encoded SHA256 digest of the key specified in encryption_key.
    ///
    /// # Note
    ///
    /// This function is the low-level setting for SSE related features.
    ///
    /// SSE related options should be set carefully to make them works.
    /// Please use `server_side_encryption_with_*` helpers if even possible.
    pub fn encryption_key_sha256(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.encryption_key_sha256 = Some(v.to_string());
        }

        self
    }

    /// Set encryption_algorithm of this backend.
    ///
    /// # Args
    ///
    /// `v`: server-side encryption algorithm. (Available values: `AES256`)
    ///
    /// # Note
    ///
    /// This function is the low-level setting for SSE related features.
    ///
    /// SSE related options should be set carefully to make them works.
    /// Please use `server_side_encryption_with_*` helpers if even possible.
    pub fn encryption_algorithm(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.encryption_algorithm = Some(v.to_string());
        }

        self
    }

    /// Enable server side encryption with customer key.
    ///
    /// As known as: CPK
    ///
    /// # Args
    ///
    /// `key`: Raw AES-256 key containing exactly 32 bytes.
    /// Building an operator returns [`ErrorKind::ConfigInvalid`] if the key
    /// has a different length.
    ///
    /// # Note
    ///
    /// Function that helps the user to set the server-side customer-provided encryption key, the key's SHA256, and the algorithm.
    /// See [Server-side encryption with customer-provided keys (CPK)](https://learn.microsoft.com/en-us/azure/storage/blobs/encryption-customer-provided-keys)
    /// for more info.
    pub fn server_side_encryption_with_customer_key(mut self, key: &[u8]) -> Self {
        // Only AES256 is supported for now
        self.config.encryption_algorithm = Some("AES256".to_string());
        self.config.encryption_key = Some(BASE64_STANDARD.encode(key));
        let key_sha256 = Sha256::digest(key);
        self.config.encryption_key_sha256 = Some(BASE64_STANDARD.encode(key_sha256));
        self
    }

    /// Set sas_token of this backend.
    ///
    /// - If sas_token is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    ///
    /// See [Grant limited access to Azure Storage resources using shared access signatures (SAS)](https://learn.microsoft.com/en-us/azure/storage/common/storage-sas-overview)
    /// for more info.
    pub fn sas_token(mut self, sas_token: &str) -> Self {
        if !sas_token.is_empty() {
            self.config.sas_token = Some(sas_token.to_string());
        }

        self
    }

    /// Replace the credential providers with a custom chain.
    ///
    /// Build the chain from [`opendal_core::reqsign_core`] and
    /// [`crate::reqsign_azure_storage`] so that its types match the ones this
    /// service is built with.
    pub fn credential_provider_chain(mut self, chain: ProvideCredentialChain<Credential>) -> Self {
        self.credential_providers = Some(chain);
        self
    }

    /// Deprecated: Azblob delete batch capability is enabled by default with Azure Blob's 256-operation batch limit.
    #[deprecated(
        since = "0.57.0",
        note = "Azblob delete batch capability is enabled by default with Azure Blob's 256-operation batch limit and this option is no longer needed."
    )]
    pub fn batch_max_operations(self, _batch_max_operations: usize) -> Self {
        self
    }

    /// Skip signature will skip loading credentials and signing requests.
    pub fn skip_signature(mut self) -> Self {
        self.config.skip_signature = true;
        self
    }

    /// from_connection_string will make a builder from connection string
    ///
    /// connection string looks like:
    ///
    /// ```txt
    /// DefaultEndpointsProtocol=http;AccountName=devstoreaccount1;
    /// AccountKey=Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==;
    /// BlobEndpoint=http://127.0.0.1:10000/devstoreaccount1;
    /// QueueEndpoint=http://127.0.0.1:10001/devstoreaccount1;
    /// TableEndpoint=http://127.0.0.1:10002/devstoreaccount1;
    /// ```
    ///
    /// Or
    ///
    /// ```txt
    /// DefaultEndpointsProtocol=https;
    /// AccountName=storagesample;
    /// AccountKey=<account-key>;
    /// EndpointSuffix=core.chinacloudapi.cn;
    /// ```
    ///
    /// For reference: [Configure Azure Storage connection strings](https://learn.microsoft.com/en-us/azure/storage/common/storage-configure-connection-string)
    ///
    /// # Note
    ///
    /// Connection strings can only configure the endpoint, account name and
    /// authentication information. Users still need to configure container name.
    pub fn from_connection_string(conn: &str) -> Result<Self> {
        let config = azure_config_from_connection_string(conn, AzureStorageService::Blob)?;

        Ok(AzblobConfig::from(config).into_builder())
    }
}

impl Builder for AzblobBuilder {
    type Config = AzblobConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        // Handle endpoint, region and container name.
        let container = match self.config.container.is_empty() {
            false => Ok(&self.config.container),
            true => Err(Error::new(ErrorKind::ConfigInvalid, "container is empty")
                .with_operation("Builder::build")
                .with_context("service", AZBLOB_SCHEME)),
        }?;
        debug!("backend use container {}", container);

        let endpoint = match &self.config.endpoint {
            Some(endpoint) => Ok(endpoint.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", AZBLOB_SCHEME)),
        }?;
        debug!("backend use endpoint {}", container);

        let account_name = self
            .config
            .account_name
            .clone()
            .or_else(|| azure_account_name_from_endpoint(endpoint.as_str()));

        if let Some(v) = &self.config.account_key {
            // Validate that account_key can be decoded as base64
            if let Err(e) = BASE64_STANDARD.decode(v) {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    format!("invalid account_key: cannot decode as base64: {e}"),
                )
                .with_operation("Builder::build")
                .with_context("service", AZBLOB_SCHEME)
                .with_context("key", "account_key"));
            }
        }

        let encryption_key = match &self.config.encryption_key {
            None => None,
            Some(v) => {
                let key = BASE64_STANDARD.decode(v).map_err(|_| {
                    Error::new(
                        ErrorKind::ConfigInvalid,
                        "encryption_key must be valid Base64",
                    )
                    .with_operation("Builder::build")
                    .with_context("service", AZBLOB_SCHEME)
                    .with_context("key", "encryption_key")
                })?;
                if key.len() != 32 {
                    return Err(Error::new(
                        ErrorKind::ConfigInvalid,
                        "encryption_key must decode to exactly 32 bytes",
                    )
                    .with_operation("Builder::build")
                    .with_context("service", AZBLOB_SCHEME)
                    .with_context("key", "encryption_key"));
                }
                Some(build_header_value(v).map_err(|err| {
                    err.with_context("key", "server_side_encryption_customer_key")
                })?)
            }
        };

        let encryption_key_sha256 = match &self.config.encryption_key_sha256 {
            None => None,
            Some(v) => Some(build_header_value(v).map_err(|err| {
                err.with_context("key", "server_side_encryption_customer_key_sha256")
            })?),
        };

        let encryption_algorithm = match &self.config.encryption_algorithm {
            None => None,
            Some(v) => {
                if v == "AES256" {
                    Some(build_header_value(v).map_err(|err| {
                        err.with_context("key", "server_side_encryption_customer_algorithm")
                    })?)
                } else {
                    return Err(Error::new(
                        ErrorKind::ConfigInvalid,
                        "encryption_algorithm value must be AES256",
                    ));
                }
            }
        };

        let ctx = Context::new().with_file_read(TokioFileRead).with_env(OsEnv);

        let mut credential_providers =
            ProvideCredentialChain::new().push(DefaultCredentialProvider::new());

        if let (Some(account_name), Some(account_key)) =
            (account_name.as_deref(), self.config.account_key.as_deref())
        {
            credential_providers = credential_providers.push_front(
                StaticCredentialProvider::new_shared_key(account_name, account_key),
            );
        }

        if let Some(sas_token) = self.config.sas_token.as_deref() {
            credential_providers =
                credential_providers.push_front(StaticCredentialProvider::new_sas_token(sas_token));
        }

        if let Some(customized_credential_chain) = self.credential_providers {
            credential_providers = customized_credential_chain;
        }

        let signer = Signer::new(
            ctx,
            credential_providers,
            RequestSigner::new().with_service_sas_permissions("racwd"),
        );

        let info = ServiceInfo::new(AZBLOB_SCHEME, &root, container);
        let capability = Capability {
            stat: true,
            stat_with_if_match: true,
            stat_with_if_none_match: true,

            read: true,

            read_with_if_match: true,
            read_with_if_none_match: true,
            read_with_override_content_disposition: true,
            read_with_if_modified_since: true,
            read_with_if_unmodified_since: true,

            write: true,
            write_can_append: true,
            write_can_empty: true,
            write_can_multi: true,
            write_with_cache_control: true,
            write_with_content_type: true,
            write_with_if_match: true,
            write_with_if_not_exists: true,
            write_with_if_none_match: true,
            write_with_user_metadata: true,

            delete: true,
            delete_with_if_match: true,
            delete_with_if_none_match: true,
            delete_max_size: Some(AZBLOB_BATCH_LIMIT),

            copy: true,
            copy_with_if_not_exists: true,
            copy_with_if_match: true,
            copy_with_if_none_match: true,
            copy_can_multi: true,
            copy_multi_min_size: Some(AZBLOB_COPY_MIN_BLOCK_SIZE),
            copy_multi_max_size: Some(AZBLOB_COPY_MAX_BLOCK_SIZE),

            list: true,
            list_with_recursive: true,

            presign: self.config.sas_token.is_some(),
            presign_stat: self.config.sas_token.is_some(),
            presign_read: self.config.sas_token.is_some(),
            presign_write: self.config.sas_token.is_some(),

            shared: true,

            ..Default::default()
        };

        Ok(AzblobBackend {
            core: Arc::new(AzblobCore {
                info,
                capability,
                root,
                endpoint,
                encryption_key,
                encryption_key_sha256,
                encryption_algorithm,
                container: self.config.container.clone(),
                skip_signature: self.config.skip_signature,
                sas_token: self.config.sas_token.clone(),
                signer,
            }),
        })
    }
}

/// Backend for azblob services.
#[derive(Debug, Clone)]
pub struct AzblobBackend {
    pub(crate) core: Arc<AzblobCore>,
}

impl Service for AzblobBackend {
    type Reader = oio::StreamReader<AzblobReader>;
    type Writer = AzblobWriters;
    type Lister = oio::PageLister<AzblobLister>;
    type Deleter = oio::BatchDeleter<AzblobDeleter>;
    type Copier = AzblobCopiers;
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
        let error_ctx = ErrorContext::new(ServiceOperation("GetBlobProperties"))
            .with_caller_condition(args.is_conditional());
        let resp = self
            .core
            .azblob_get_blob_properties(ctx, path, &args)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let headers = resp.headers();
                let mut meta = parse_into_metadata(path, headers)?.into_builder();
                if let Some(version_id) = parse_header_to_str(headers, X_MS_VERSION_ID)? {
                    meta.version(version_id);
                }

                let user_meta = parse_prefixed_headers(headers, X_MS_META_PREFIX);
                if !user_meta.is_empty() {
                    meta.user_metadata(user_meta);
                }

                Ok(RpStat::new(meta.build()))
            }
            _ => Err(parse_error(error_ctx, resp)),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<AzblobReader> = {
            Ok(oio::StreamReader::new(AzblobReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: AzblobWriters = {
            let w = AzblobWriter::new(
                self.core.clone(),
                ctx.clone(),
                args.clone(),
                path.to_string(),
            );
            let w = if args.append() {
                AzblobWriters::Two(oio::AppendWriter::new(w))
            } else {
                AzblobWriters::One(oio::BlockWriter::new(
                    ctx.executor().clone(),
                    w,
                    args.concurrent(),
                ))
            };

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::BatchDeleter<AzblobDeleter> = {
            Ok(oio::BatchDeleter::new(
                AzblobDeleter::new(self.core.clone(), ctx.clone()),
                self.core.capability.delete_max_size,
            ))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<AzblobLister> = {
            let l = AzblobLister::new(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
                args.recursive(),
                args.limit(),
            );

            Ok(oio::PageLister::new(l))
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
        let output: AzblobCopiers = {
            let copier = new_azblob_copier(self.core.clone(), ctx, from, to, args)?;
            Ok(copier)
        }?;

        Ok(output)
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
        let req = match args.operation() {
            PresignOperation::Stat(v) => self.core.azblob_head_blob_request(path, v),
            PresignOperation::Read(range, v) => self.core.azblob_get_blob_request(path, *range, v),
            PresignOperation::Write(v) => {
                self.core
                    .azblob_put_blob_request(path, None, v, Buffer::new())
            }
            PresignOperation::Delete(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "operation is not supported",
            )),
            _ => Err(Error::new(
                ErrorKind::Unsupported,
                "presign operation is not supported",
            )),
        };

        let req = req?;
        let req = self.core.sign_query(ctx, req).await?;

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
    fn invalid_encoded_customer_keys_are_rejected() {
        for key in [
            String::new(),
            "not-base64!".to_string(),
            BASE64_STANDARD.encode([42; 16]),
            BASE64_STANDARD.encode([42; 31]),
            BASE64_STANDARD.encode([42; 33]),
            BASE64_STANDARD.encode([42; 64]),
        ] {
            let config = AzblobConfig {
                container: "test".to_string(),
                endpoint: Some("https://test.blob.core.windows.net".to_string()),
                encryption_key: Some(key.clone()),
                ..Default::default()
            };
            let err = Operator::from_config(config).expect_err("invalid key must fail to build");
            assert_eq!(err.kind(), ErrorKind::ConfigInvalid);
            let message = format!("{err:?}");
            assert!(message.contains("encryption_key"));
            if !key.is_empty() {
                assert!(!message.contains(&key));
            }
        }
    }

    #[test]
    fn invalid_raw_customer_keys_are_rejected() {
        for len in [0, 16, 31, 33, 64] {
            let builder = AzblobBuilder::default()
                .container("test")
                .endpoint("https://test.blob.core.windows.net")
                .server_side_encryption_with_customer_key(&vec![42; len]);
            let err = Operator::new(builder).expect_err("invalid key must fail to build");
            assert_eq!(err.kind(), ErrorKind::ConfigInvalid);
        }
    }

    #[test]
    fn valid_customer_keys_build() {
        let key = [42; 32];
        let builder = AzblobBuilder::default()
            .container("test")
            .endpoint("https://test.blob.core.windows.net")
            .server_side_encryption_with_customer_key(&key);
        Operator::new(builder).expect("32-byte raw key must build");

        let config = AzblobConfig {
            container: "test".to_string(),
            endpoint: Some("https://test.blob.core.windows.net".to_string()),
            encryption_key: Some(BASE64_STANDARD.encode(key)),
            encryption_key_sha256: Some(BASE64_STANDARD.encode(Sha256::digest(key))),
            encryption_algorithm: Some("AES256".to_string()),
            ..Default::default()
        };
        Operator::from_config(config).expect("Base64-encoded 32-byte key must build");
    }
}
