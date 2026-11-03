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
use reqsign_azure_storage::DefaultCredentialProvider;
use reqsign_azure_storage::RequestSigner;
use reqsign_azure_storage::StaticCredentialProvider;
use reqsign_core::Context;
use reqsign_core::Env as _;
use reqsign_core::OsEnv;
use reqsign_core::Signer;
use reqsign_core::StaticEnv;
use reqsign_file_read_tokio::TokioFileRead;

use super::AZFILE_SCHEME;
use super::config::AzfileConfig;
use super::core::X_MS_META_PREFIX;
use super::core::parse_error;
use super::core::{AzfileCore, ErrorContext};
use super::deleter::AzfileDeleter;
use super::lister::AzfileLister;
use super::reader::*;
use super::writer::AzfileWriter;
use super::writer::AzfileWriters;
use opendal_core::raw::*;
use opendal_core::*;
use opendal_service_azure_common::{
    AzureStorageConfig as AzureConnectionConfig, AzureStorageService,
    azure_account_name_from_endpoint, azure_config_from_connection_string,
};

impl From<AzureConnectionConfig> for AzfileConfig {
    fn from(config: AzureConnectionConfig) -> Self {
        AzfileConfig {
            account_name: config.account_name,
            account_key: config.account_key,
            sas_token: config.sas_token,
            endpoint: config.endpoint,
            root: None,                // root is not part of AzureStorageConfig
            share_name: String::new(), // share_name is not part of AzureStorageConfig
        }
    }
}

/// Azure File services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct AzfileBuilder {
    pub(super) config: AzfileConfig,
}

impl Debug for AzfileBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzfileBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AzfileBuilder {
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

    /// Set endpoint of this backend.
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

    /// Set file share name of this backend.
    ///
    /// # Notes
    /// You can find more about from: <https://learn.microsoft.com/en-us/rest/api/storageservices/operations-on-shares--file-service>
    pub fn share_name(mut self, share_name: &str) -> Self {
        if !share_name.is_empty() {
            self.config.share_name = share_name.to_string();
        }

        self
    }

    /// Create a new `AfileBuilder` instance from an [Azure Storage connection string][1].
    ///
    /// [1]: https://learn.microsoft.com/en-us/azure/storage/common/storage-configure-connection-string
    ///
    /// # Example
    /// ```
    /// use opendal_core::Builder;
    /// use opendal_service_azfile::Azfile;
    ///
    /// let conn_str = "AccountName=example;DefaultEndpointsProtocol=https;EndpointSuffix=core.windows.net";
    ///
    /// let mut config = Azfile::from_connection_string(&conn_str)
    ///     .unwrap()
    ///     // Add additional configuration if needed
    ///     .share_name("myShare")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn from_connection_string(conn_str: &str) -> Result<Self> {
        let config = azure_config_from_connection_string(conn_str, AzureStorageService::File)?;

        Ok(AzfileConfig::from(config).into_builder())
    }
}

impl Builder for AzfileBuilder {
    type Config = AzfileConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let endpoint = match &self.config.endpoint {
            Some(endpoint) => Ok(endpoint.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", AZFILE_SCHEME)),
        }?;
        debug!("backend use endpoint {}", endpoint);

        let account_name_option = self
            .config
            .account_name
            .clone()
            .or_else(|| azure_account_name_from_endpoint(endpoint.as_str()));

        let account_name = match account_name_option {
            Some(account_name) => Ok(account_name),
            None => Err(
                Error::new(ErrorKind::ConfigInvalid, "account_name is empty")
                    .with_operation("Builder::build")
                    .with_context("service", AZFILE_SCHEME),
            ),
        }?;

        let mut envs = std::collections::HashMap::new();
        envs.insert("AZBLOB_ACCOUNT_NAME".to_string(), account_name.clone());
        envs.insert(
            "AZURE_STORAGE_ACCOUNT_NAME".to_string(),
            account_name.clone(),
        );

        if let Some(v) = &self.config.account_key {
            envs.insert("AZBLOB_ACCOUNT_KEY".to_string(), v.clone());
            envs.insert("AZURE_STORAGE_ACCOUNT_KEY".to_string(), v.clone());
        }
        if let Some(v) = &self.config.sas_token {
            envs.insert("AZURE_STORAGE_SAS_TOKEN".to_string(), v.clone());
        }

        let os_env = OsEnv;
        let ctx = Context::new()
            .with_file_read(TokioFileRead)
            .with_env(StaticEnv {
                home_dir: os_env.home_dir(),
                envs,
            });

        let mut credential = DefaultCredentialProvider::new();
        if let Some(account_key) = self.config.account_key.as_deref() {
            credential = credential.push_front(StaticCredentialProvider::new_shared_key(
                &account_name,
                account_key,
            ));
        }
        if let Some(sas_token) = self.config.sas_token.as_deref() {
            credential = credential.push_front(StaticCredentialProvider::new_sas_token(sas_token));
        }

        let sign_ctx = ctx;
        let signer = Signer::new(sign_ctx.clone(), credential, RequestSigner::new());

        let info = ServiceInfo::new(AZFILE_SCHEME, &root, "");
        let capability = Capability {
            stat: true,

            read: true,

            write: true,
            write_with_user_metadata: true,
            // Azure Files limits each Put Range update to 4 MiB.
            // ref: <https://learn.microsoft.com/en-us/rest/api/storageservices/put-range>
            write_total_max_size: Some(4 * 1024 * 1024),

            create_dir: true,
            delete: true,
            rename: true,

            list: true,

            shared: true,

            ..Default::default()
        };

        Ok(AzfileBackend {
            core: Arc::new(AzfileCore {
                info,
                capability,
                root,
                endpoint,
                signer,
                sign_ctx,
                share_name: self.config.share_name.clone(),
            }),
        })
    }
}

/// Backend for azfile services.
#[derive(Debug, Clone)]
pub struct AzfileBackend {
    pub(crate) core: Arc<AzfileCore>,
}

impl Service for AzfileBackend {
    type Reader = oio::StreamReader<AzfileReader>;
    type Writer = AzfileWriters;
    type Lister = oio::PageLister<AzfileLister>;
    type Deleter = oio::OneShotDeleter<AzfileDeleter>;
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
        self.core.ensure_parent_dir_exists(ctx, path).await?;
        let resp = self.core.azfile_create_dir(ctx, path).await?;
        let status = resp.status();

        match status {
            StatusCode::CREATED => Ok(RpCreateDir::default()),
            _ => {
                // we cannot just check status code because 409 Conflict has two meaning:
                // 1. If a directory by the same name is being deleted when Create Directory is called, the server returns status code 409 (Conflict)
                // 2. If a directory or file with the same name already exists, the operation fails with status code 409 (Conflict).
                // but we just need case 2 (already exists)
                // ref: https://learn.microsoft.com/en-us/rest/api/storageservices/create-directory
                if resp
                    .headers()
                    .get("x-ms-error-code")
                    .map(|value| value.to_str().unwrap_or(""))
                    .unwrap_or_else(|| "")
                    == "ResourceAlreadyExists"
                {
                    Ok(RpCreateDir::default())
                } else {
                    Err(parse_error(
                        ErrorContext::new(ServiceOperation("CreateDirectory")),
                        resp,
                    ))
                }
            }
        }
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let resp = if path.ends_with('/') {
            self.core.azfile_get_directory_properties(ctx, path).await?
        } else {
            self.core.azfile_get_file_properties(ctx, path).await?
        };

        let status = resp.status();
        match status {
            StatusCode::OK => {
                let headers = resp.headers();
                let mut meta = parse_into_metadata(path, headers)?.into_builder();
                let user_meta = parse_prefixed_headers(headers, X_MS_META_PREFIX);
                if !user_meta.is_empty() {
                    meta.user_metadata(user_meta);
                }
                Ok(RpStat::new(meta.build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(if path.ends_with('/') {
                    ServiceOperation("GetDirectoryProperties")
                } else {
                    ServiceOperation("GetFileProperties")
                }),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<AzfileReader> = {
            Ok(oio::StreamReader::new(AzfileReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: AzfileWriters = {
            let w = AzfileWriter::new(
                self.core.clone(),
                ctx.clone(),
                args.clone(),
                path.to_string(),
            );
            let w = if args.append() {
                AzfileWriters::Two(oio::AppendWriter::new(w))
            } else {
                AzfileWriters::One(oio::OneShotWriter::new(w))
            };
            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<AzfileDeleter> = {
            Ok(oio::OneShotDeleter::new(AzfileDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<AzfileLister> = {
            let l = AzfileLister::new(
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
        _: OpRename,
    ) -> Result<RpRename> {
        self.core.ensure_parent_dir_exists(ctx, to).await?;
        let resp = self.core.azfile_rename(ctx, from, to).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => Ok(RpRename::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("Rename")),
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
