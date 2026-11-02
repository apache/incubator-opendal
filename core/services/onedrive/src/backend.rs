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
use super::core::{ErrorContext, OneDriveCore};
use super::deleter::OneDriveDeleter;
use super::lister::OneDriveLister;
use super::reader::*;
use super::writer::OneDriveWriter;

use std::fmt::Debug;

use asyncband::mutex::Mutex;
use log::debug;

use super::ONEDRIVE_SCHEME;
use super::config::OnedriveConfig;
use super::core::OneDriveSigner;

/// Microsoft [OneDrive](https://onedrive.com) backend support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct OnedriveBuilder {
    pub(super) config: OnedriveConfig,
}

impl Debug for OnedriveBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnedriveBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl OnedriveBuilder {
    /// Set root path of OneDrive folder.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set the access token for a time limited access to Microsoft Graph API (also OneDrive).
    ///
    /// Microsoft Graph API uses a typical OAuth 2.0 flow for authentication and authorization.
    /// You can get a access token from [Microsoft Graph Explore](https://developer.microsoft.com/en-us/graph/graph-explorer).
    ///
    /// # Note
    ///
    /// - An access token is short-lived.
    /// - Use a refresh_token if you want to use OneDrive API for an extended period of time.
    pub fn access_token(mut self, access_token: &str) -> Self {
        self.config.access_token = Some(access_token.to_string());
        self
    }

    /// Set the refresh token for long term access to Microsoft Graph API.
    ///
    /// OpenDAL will use a refresh token to maintain a fresh access token automatically.
    ///
    /// # Note
    ///
    /// - A refresh token is available through a OAuth 2.0 flow, with an additional scope `offline_access`.
    pub fn refresh_token(mut self, refresh_token: &str) -> Self {
        self.config.refresh_token = Some(refresh_token.to_string());
        self
    }

    /// Set the client_id for a Microsoft Graph API application (available though Azure's registration portal)
    ///
    /// Required when using the refresh token.
    pub fn client_id(mut self, client_id: &str) -> Self {
        self.config.client_id = Some(client_id.to_string());
        self
    }

    /// Set the client_secret for a Microsoft Graph API application
    ///
    /// Required for Web app when using the refresh token.
    /// Don't use a client secret when use in a native app since the native app can't store the secret reliably.
    pub fn client_secret(mut self, client_secret: &str) -> Self {
        self.config.client_secret = Some(client_secret.to_string());
        self
    }

    /// Deprecated: OneDrive supports version listing without this option.
    #[deprecated(
        since = "0.57.0",
        note = "OneDrive supports version listing without this option."
    )]
    pub fn enable_versioning(self, _enabled: bool) -> Self {
        self
    }
}

impl Builder for OnedriveBuilder {
    type Config = OnedriveConfig;

    fn build(self) -> Result<impl Service> {
        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let info = ServiceInfo::new(ONEDRIVE_SCHEME, &root, "");
        let capability = Capability {
            read: true,
            read_with_suffix: true,
            read_with_if_none_match: true,

            write: true,
            write_with_if_match: true,
            // disable because usize is too small on armhf and other arch to represent more than 4GB
            #[cfg(target_pointer_width = "64")]
            // Read more at https://support.microsoft.com/en-us/office/restrictions-and-limitations-in-onedrive-and-sharepoint-64883a5d-228e-48f5-b3d2-eb39e07630fa#individualfilesize
            write_total_max_size: Some(250 * 1024 * 1024 * 1024), // 250GB
            copy: true,
            rename: true,

            stat: true,
            stat_with_if_none_match: true,
            // Microsoft Graph doesn't preserve complete metadata for previous
            // file versions, so OneDrive can't implement stat_with_version.
            // See https://learn.microsoft.com/en-us/graph/api/driveitem-list-versions?view=graph-rest-1.0#remarks
            delete: true,
            create_dir: true,

            list: true,
            list_with_limit: true,
            list_with_start_after: true,
            list_with_versions: true,

            shared: true,

            ..Default::default()
        };

        let accessor_info = info;
        let mut signer = OneDriveSigner::new();

        // Requires OAuth 2.0 tokens:
        // - `access_token` (the short-lived token)
        // - `refresh_token` flow (the long term token)
        // to be mutually exclusive for setting up for implementation simplicity
        match (self.config.access_token, self.config.refresh_token) {
            (Some(access_token), None) => {
                signer.access_token = access_token;
                signer.expires_in = Timestamp::MAX;
            }
            (None, Some(refresh_token)) => {
                let client_id = self.config.client_id.ok_or_else(|| {
                    Error::new(
                        ErrorKind::ConfigInvalid,
                        "client_id must be set when refresh_token is set",
                    )
                    .with_context("service", ONEDRIVE_SCHEME)
                })?;

                signer.refresh_token = refresh_token;
                signer.client_id = client_id;
                if let Some(client_secret) = self.config.client_secret {
                    signer.client_secret = client_secret;
                }
            }
            (Some(_), Some(_)) => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "access_token and refresh_token cannot be set at the same time",
                )
                .with_context("service", ONEDRIVE_SCHEME));
            }
            (None, None) => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "access_token or refresh_token must be set",
                )
                .with_context("service", ONEDRIVE_SCHEME));
            }
        };

        let core = Arc::new(OneDriveCore {
            info: accessor_info,
            capability,
            root,
            signer: Arc::new(Mutex::new(signer)),
        });

        Ok(OnedriveBackend { core })
    }
}

#[derive(Clone, Debug)]
pub struct OnedriveBackend {
    pub core: Arc<OneDriveCore>,
}

impl Service for OnedriveBackend {
    type Reader = oio::StreamReader<OnedriveReader>;
    type Writer = oio::OneShotWriter<OneDriveWriter>;
    type Lister = oio::PageLister<OneDriveLister>;
    type Deleter = oio::OneShotDeleter<OneDriveDeleter>;
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
        ctx: &OperationContext,
        path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        if path == "/" {
            // skip, the root path exists in the personal OneDrive.
            return Ok(RpCreateDir::default());
        }

        let response = self.core.onedrive_create_dir(ctx, path).await?;
        match response.status() {
            StatusCode::CREATED | StatusCode::OK => Ok(RpCreateDir::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("CreateFolder")),
                response,
            )),
        }
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let meta = self.core.onedrive_stat(ctx, path, args).await?;

        Ok(RpStat::new(meta))
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<OnedriveReader> = {
            Ok(oio::StreamReader::new(OnedriveReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: oio::OneShotWriter<OneDriveWriter> = {
            Ok(oio::OneShotWriter::new(OneDriveWriter::new(
                self.core.clone(),
                ctx.clone(),
                args,
                path.to_string(),
            )))
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<OneDriveDeleter> = {
            Ok(oio::OneShotDeleter::new(OneDriveDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
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
        let backend = self.clone();
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();
        let source_content_length_hint = args.source_content_length_hint();

        Ok(oio::OneShotCopier::new(async move {
            let source_size = match source_content_length_hint {
                Some(size) => size,
                None => backend
                    .stat(&ctx, &from, OpStat::default())
                    .await?
                    .into_metadata()
                    .content_length(),
            };

            let monitor_url = core.initialize_copy(&ctx, &from, &to).await?;
            core.wait_until_complete(&ctx, monitor_url).await?;
            Ok(MetadataBuilder::file(source_size).build())
        }))
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        if from == to {
            return Ok(RpRename::default());
        }

        self.core.onedrive_move(ctx, from, to).await?;

        Ok(RpRename::default())
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<OneDriveLister> = {
            let l = OneDriveLister::new(path.to_string(), self.core.clone(), ctx.clone(), &args);
            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
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
