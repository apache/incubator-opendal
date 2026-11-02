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
use log::debug;
use reqsign_azure_storage::ClientSecretCredentialProvider;
use reqsign_azure_storage::Credential;
use reqsign_azure_storage::DefaultCredentialProvider;
use reqsign_azure_storage::RequestSigner;
use reqsign_azure_storage::StaticCredentialProvider;
use reqsign_core::Context;
use reqsign_core::OsEnv;
use reqsign_core::ProvideCredentialChain;
use reqsign_core::Signer;
use reqsign_file_read_tokio::TokioFileRead;

use super::AZDLS_SCHEME;
use super::config::AzdlsConfig;
use super::core::DIRECTORY;
use super::core::parse_error;
use super::core::{AzdlsCore, ErrorContext};
use super::deleter::AzdlsDeleter;
use super::lister::AzdlsLister;
use super::reader::*;
use super::writer::AzdlsLazyPositionWriter;
use super::writer::AzdlsWriter;
use super::writer::AzdlsWriters;
use opendal_core::raw::*;
use opendal_core::*;
use opendal_service_azure_common::{
    AzureStorageConfig as AzureConnectionConfig, AzureStorageService,
    azure_account_name_from_endpoint, azure_config_from_connection_string,
};

impl From<AzureConnectionConfig> for AzdlsConfig {
    fn from(config: AzureConnectionConfig) -> Self {
        AzdlsConfig {
            endpoint: config.endpoint,
            account_name: config.account_name,
            account_key: config.account_key,
            client_secret: config.client_secret,
            tenant_id: config.tenant_id,
            client_id: config.client_id,
            sas_token: config.sas_token,
            authority_host: config.authority_host,
            ..Default::default()
        }
    }
}

/// Azure Data Lake Storage Gen2 Support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct AzdlsBuilder {
    pub(super) config: AzdlsConfig,
    pub(super) credential_providers: Option<ProvideCredentialChain<Credential>>,
}

impl Debug for AzdlsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzdlsBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AzdlsBuilder {
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

    /// Set filesystem name of this backend.
    pub fn filesystem(mut self, filesystem: &str) -> Self {
        self.config.filesystem = filesystem.to_string();

        self
    }

    /// Set endpoint of this backend.
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

    /// Set client_secret of this backend.
    ///
    /// - If client_secret is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    /// - required for client_credentials authentication
    pub fn client_secret(mut self, client_secret: &str) -> Self {
        if !client_secret.is_empty() {
            self.config.client_secret = Some(client_secret.to_string());
        }

        self
    }

    /// Set tenant_id of this backend.
    ///
    /// - If tenant_id is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    /// - required for client_credentials authentication
    pub fn tenant_id(mut self, tenant_id: &str) -> Self {
        if !tenant_id.is_empty() {
            self.config.tenant_id = Some(tenant_id.to_string());
        }

        self
    }

    /// Set client_id of this backend.
    ///
    /// - If client_id is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    /// - required for client_credentials authentication
    pub fn client_id(mut self, client_id: &str) -> Self {
        if !client_id.is_empty() {
            self.config.client_id = Some(client_id.to_string());
        }

        self
    }

    /// Set the sas_token of this backend.
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

    /// Set authority_host of this backend.
    ///
    /// - If authority_host is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    /// - default value: `https://login.microsoftonline.com`
    pub fn authority_host(mut self, authority_host: &str) -> Self {
        if !authority_host.is_empty() {
            self.config.authority_host = Some(authority_host.to_string());
        }

        self
    }

    /// Create a new `AzdlsBuilder` instance from an [Azure Storage connection string][1].
    ///
    /// [1]: https://learn.microsoft.com/en-us/azure/storage/common/storage-configure-connection-string
    ///
    /// # Example
    /// ```
    /// use opendal_core::Builder;
    /// use opendal_service_azdls::Azdls;
    ///
    /// let conn_str = "AccountName=example;DefaultEndpointsProtocol=https;EndpointSuffix=core.windows.net";
    ///
    /// let mut config = Azdls::from_connection_string(&conn_str)
    ///     .unwrap()
    ///     // Add additional configuration if needed
    ///     .filesystem("myFilesystem")
    ///     .client_id("myClientId")
    ///     .client_secret("myClientSecret")
    ///     .tenant_id("myTenantId")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn from_connection_string(conn_str: &str) -> Result<Self> {
        let config = azure_config_from_connection_string(conn_str, AzureStorageService::Adls)?;

        Ok(AzdlsConfig::from(config).into_builder())
    }

    /// Enable or disable HNS (Hierarchical Namespace) for this backend.
    pub fn enable_hns(mut self, enable: bool) -> Self {
        self.config.enable_hns = enable;
        self
    }
}

impl Builder for AzdlsBuilder {
    type Config = AzdlsConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        // Handle endpoint, region and container name.
        let filesystem = match self.config.filesystem.is_empty() {
            false => Ok(&self.config.filesystem),
            true => Err(Error::new(ErrorKind::ConfigInvalid, "filesystem is empty")
                .with_operation("Builder::build")
                .with_context("service", AZDLS_SCHEME)),
        }?;
        debug!("backend use filesystem {}", filesystem);

        let endpoint = match &self.config.endpoint {
            Some(endpoint) => Ok(endpoint.clone().trim_end_matches('/').to_string()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", AZDLS_SCHEME)),
        }?;
        debug!("backend use endpoint {}", endpoint);

        let account_name = self
            .config
            .account_name
            .clone()
            .or_else(|| azure_account_name_from_endpoint(endpoint.as_str()));

        let ctx = Context::new().with_file_read(TokioFileRead).with_env(OsEnv);

        let mut client_secret = ClientSecretCredentialProvider::new();
        if let Some(v) = &self.config.tenant_id {
            client_secret = client_secret.with_tenant_id(v);
        }
        if let Some(v) = &self.config.client_id {
            client_secret = client_secret.with_client_id(v);
        }
        if let Some(v) = &self.config.client_secret {
            client_secret = client_secret.with_client_secret(v);
        }
        if let Some(v) = &self.config.authority_host {
            client_secret = client_secret.with_authority_host(v);
        }

        let mut credential_providers = ProvideCredentialChain::new().push(
            DefaultCredentialProvider::builder()
                .client_secret(client_secret)
                .build(),
        );

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

        let sign_ctx = ctx;
        let signer = Signer::new(sign_ctx.clone(), credential_providers, RequestSigner::new());

        let info = ServiceInfo::new(AZDLS_SCHEME, &root, filesystem);
        let capability = Capability {
            stat: true,
            stat_with_if_match: true,
            stat_with_if_none_match: true,
            stat_with_if_modified_since: true,
            stat_with_if_unmodified_since: true,

            read: true,
            read_with_if_match: true,
            read_with_if_none_match: true,
            read_with_if_modified_since: true,
            read_with_if_unmodified_since: true,

            write: true,
            write_can_append: true,
            write_can_multi: true,
            write_with_if_none_match: true,
            write_with_if_not_exists: true,
            write_with_user_metadata: true,

            create_dir: true,

            delete: true,
            delete_with_if_match: true,
            delete_with_recursive: true,

            rename: true,

            list: true,

            shared: true,

            ..Default::default()
        };

        Ok(AzdlsBackend {
            core: Arc::new(AzdlsCore {
                info,
                capability,
                filesystem: self.config.filesystem.clone(),
                root,
                endpoint,
                enable_hns: self.config.enable_hns,
                signer,
                sign_ctx,
            }),
        })
    }
}

/// Backend for azblob services.
#[derive(Debug, Clone)]
pub struct AzdlsBackend {
    pub(crate) core: Arc<AzdlsCore>,
}

impl Service for AzdlsBackend {
    type Reader = oio::StreamReader<AzdlsReader>;
    type Writer = AzdlsWriters;
    type Lister = oio::PageLister<AzdlsLister>;
    type Deleter = oio::OneShotDeleter<AzdlsDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info.clone()
    }

    fn capability(&self) -> Capability {
        self.core.capability
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let resp = self
            .core
            .azdls_create(ctx, path, DIRECTORY, &OpWrite::default())
            .await?;

        let status = resp.status();
        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(RpCreateDir::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateDirectory")),
                resp,
            )),
        }
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        // Stat root always returns a DIR.
        // TODO: include metadata for the root (#4746)
        if path == "/" {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        let metadata = self.core.azdls_stat_metadata(ctx, path, &args).await?;
        Ok(RpStat::new(metadata))
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<AzdlsReader> = {
            Ok(oio::StreamReader::new(AzdlsReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: AzdlsWriters = {
            if args.append() {
                let w = AzdlsWriter::new(
                    self.core.clone(),
                    ctx.clone(),
                    args.clone(),
                    path.to_string(),
                );
                Ok(AzdlsWriters::Two(oio::AppendWriter::new(w)))
            } else {
                let w = AzdlsWriter::new(
                    self.core.clone(),
                    ctx.clone(),
                    args.clone(),
                    path.to_string(),
                );
                let w = oio::PositionWriter::new(
                    ctx.executor().clone(),
                    AzdlsLazyPositionWriter::new(w),
                    args.concurrent(),
                );
                Ok(AzdlsWriters::One(w))
            }
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<AzdlsDeleter> = {
            Ok(oio::OneShotDeleter::new(AzdlsDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<AzdlsLister> = {
            let l = AzdlsLister::new(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
                args.limit(),
            );

            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        _ctx: &OperationContext,
        _from: &str,
        _to: &str,
        _args: OpCopy,
    ) -> Result<Self::Copier> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        if let Some(resp) = self.core.azdls_ensure_parent_path(ctx, to).await? {
            let status = resp.status();
            match status {
                StatusCode::CREATED | StatusCode::CONFLICT => {}
                _ => {
                    return Err(parse_error(
                        ErrorContext::new(ServiceOperation("CreateDirectory")),
                        resp,
                    ));
                }
            }
        }

        let resp = self.core.azdls_rename(ctx, from, to).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED => Ok(RpRename::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("RenamePath")),
                resp,
            )),
        }
    }

    async fn presign(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpPresign,
    ) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}
