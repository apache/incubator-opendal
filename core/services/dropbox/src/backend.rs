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

use bytes::Buf;
use http::StatusCode;

use super::core::*;
use super::deleter::DropboxDeleter;
use super::lister::DropboxLister;
use super::reader::*;
use super::writer::DropboxWriter;
use opendal_core::raw::*;
use opendal_core::*;

use std::fmt::Debug;

use asyncband::mutex::Mutex;

use super::DROPBOX_SCHEME;
use super::config::DropboxConfig;
use super::core::DropboxCore;
use super::core::DropboxSigner;

/// [Dropbox](https://www.dropbox.com/) backend support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct DropboxBuilder {
    pub(super) config: DropboxConfig,
}

impl Debug for DropboxBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Builder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl DropboxBuilder {
    /// Set the root directory for dropbox.
    ///
    /// Default to `/` if not set.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Access token is used for temporary access to the Dropbox API.
    ///
    /// You can get the access token from [Dropbox App Console](https://www.dropbox.com/developers/apps)
    ///
    /// NOTE: this token will be expired in 4 hours.
    /// If you are trying to use the Dropbox service in a long time, please set a refresh_token instead.
    pub fn access_token(mut self, access_token: &str) -> Self {
        self.config.access_token = Some(access_token.to_string());
        self
    }

    /// Refresh token is used for long term access to the Dropbox API.
    ///
    /// You can get the refresh token via OAuth 2.0 Flow of Dropbox.
    ///
    /// OpenDAL will use this refresh token to get a new access token when the old one is expired.
    pub fn refresh_token(mut self, refresh_token: &str) -> Self {
        self.config.refresh_token = Some(refresh_token.to_string());
        self
    }

    /// Set the client id for Dropbox.
    ///
    /// This is required for OAuth 2.0 Flow to refresh the access token.
    pub fn client_id(mut self, client_id: &str) -> Self {
        self.config.client_id = Some(client_id.to_string());
        self
    }

    /// Set the client secret for Dropbox.
    ///
    /// This is required for OAuth 2.0 Flow with refresh the access token.
    pub fn client_secret(mut self, client_secret: &str) -> Self {
        self.config.client_secret = Some(client_secret.to_string());
        self
    }
}

impl Builder for DropboxBuilder {
    type Config = DropboxConfig;

    fn build(self) -> Result<impl Service> {
        let root = normalize_root(&self.config.root.unwrap_or_default());

        let signer = match (self.config.access_token, self.config.refresh_token) {
            (Some(access_token), None) => DropboxSigner {
                access_token,
                // We will never expire user specified token.
                expires_in: Timestamp::MAX,
                ..Default::default()
            },
            (None, Some(refresh_token)) => {
                let client_id = self.config.client_id.ok_or_else(|| {
                    Error::new(
                        ErrorKind::ConfigInvalid,
                        "client_id must be set when refresh_token is set",
                    )
                    .with_context("service", DROPBOX_SCHEME)
                })?;
                let client_secret = self.config.client_secret.ok_or_else(|| {
                    Error::new(
                        ErrorKind::ConfigInvalid,
                        "client_secret must be set when refresh_token is set",
                    )
                    .with_context("service", DROPBOX_SCHEME)
                })?;

                DropboxSigner {
                    refresh_token,
                    client_id,
                    client_secret,
                    ..Default::default()
                }
            }
            (Some(_), Some(_)) => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "access_token and refresh_token can not be set at the same time",
                )
                .with_context("service", DROPBOX_SCHEME));
            }
            (None, None) => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "access_token or refresh_token must be set",
                )
                .with_context("service", DROPBOX_SCHEME));
            }
        };

        Ok(DropboxBackend {
            core: Arc::new(DropboxCore {
                info: ServiceInfo::new(DROPBOX_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,

                    read: true,
                    read_with_suffix: true,

                    write: true,

                    create_dir: true,

                    delete: true,

                    list: true,
                    list_with_recursive: true,

                    copy: true,

                    rename: true,

                    shared: true,

                    ..Default::default()
                },
                root,
                signer: Arc::new(Mutex::new(signer)),
            }),
        })
    }
}

#[derive(Clone, Debug)]
pub struct DropboxBackend {
    pub core: Arc<DropboxCore>,
}

impl Service for DropboxBackend {
    type Reader = oio::StreamReader<DropboxReader>;
    type Writer = oio::OneShotWriter<DropboxWriter>;
    type Lister = oio::PageLister<DropboxLister>;
    type Deleter = oio::OneShotDeleter<DropboxDeleter>;
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
        // Check if the folder already exists.
        let resp = self.core.dropbox_get_metadata(ctx, path).await?;
        if StatusCode::OK == resp.status() {
            let bytes = resp.into_body();
            let decoded_response: DropboxMetadataResponse =
                serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;
            if "folder" == decoded_response.tag {
                return Ok(RpCreateDir::default());
            }
            if "file" == decoded_response.tag {
                return Err(Error::new(
                    ErrorKind::NotADirectory,
                    format!("it's not a directory {path}"),
                ));
            }
        }

        let res = self.core.dropbox_create_folder(ctx, path).await?;
        Ok(res)
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let resp = self.core.dropbox_get_metadata(ctx, path).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let bytes = resp.into_body();
                let decoded_response: DropboxMetadataResponse =
                    serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;
                let entry_mode: EntryMode = match decoded_response.tag.as_str() {
                    "file" => EntryMode::FILE,
                    "folder" => EntryMode::DIR,
                    _ => EntryMode::Unknown,
                };

                let mut metadata = match entry_mode {
                    EntryMode::FILE => {
                        MetadataBuilder::file(decoded_response.size.ok_or_else(|| {
                            Error::new(
                                ErrorKind::Unexpected,
                                format!("no size found for file {path}"),
                            )
                        })?)
                    }
                    EntryMode::DIR => MetadataBuilder::dir(),
                    EntryMode::Unknown => MetadataBuilder::unknown(),
                };
                // Only set last_modified and size if entry_mode is FILE, because Dropbox API
                // returns last_modified and size only for files.
                // FYI: https://www.dropbox.com/developers/documentation/http/documentation#files-get_metadata
                if entry_mode == EntryMode::FILE {
                    let date_utc_last_modified =
                        decoded_response.client_modified.parse::<Timestamp>()?;
                    metadata.last_modified(date_utc_last_modified);
                }
                Ok(RpStat::new(metadata.build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetMetadata")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<DropboxReader> = {
            Ok(oio::StreamReader::new(DropboxReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: oio::OneShotWriter<DropboxWriter> = {
            Ok(oio::OneShotWriter::new(DropboxWriter::new(
                self.core.clone(),
                ctx.clone(),
                args,
                String::from(path),
            )))
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<DropboxDeleter> = {
            Ok(oio::OneShotDeleter::new(DropboxDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<DropboxLister> = {
            Ok(oio::PageLister::new(DropboxLister::new(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
                args.recursive(),
                args.limit(),
            )))
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _: OpCopy,
    ) -> Result<Self::Copier> {
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();

        Ok(oio::OneShotCopier::new(async move {
            let resp = core.dropbox_copy(&ctx, &from, &to).await?;
            let status = resp.status();

            match status {
                StatusCode::OK => {
                    let decoded_response: DropboxMetadataResponse =
                        serde_json::from_reader(resp.into_body().reader())
                            .map_err(new_json_deserialize_error)?;
                    DropboxWriter::parse_metadata(decoded_response)
                }
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyFile")),
                    resp,
                )),
            }
        }))
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _: OpRename,
    ) -> Result<RpRename> {
        let resp = self.core.dropbox_move(ctx, from, to).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(RpRename::default()),
            _ => {
                let err = parse_error(ErrorContext::new(ServiceOperation("MoveFile")), resp);
                match err.kind() {
                    ErrorKind::NotFound => Ok(RpRename::default()),
                    _ => Err(err),
                }
            }
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
