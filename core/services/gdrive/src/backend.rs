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

use bytes::Buf;
use http::StatusCode;

use super::core::GdriveFile;
use super::core::GdriveRecentPathState;
use super::core::normalize_dir_path;
use super::core::parse_error;
use super::core::{ErrorContext, GdriveCore};
use super::deleter::GdriveDeleter;
use super::lister::GdriveFlatLister;
use super::lister::GdriveLister;
use super::reader::*;
use super::writer::GdriveWriter;
use opendal_core::raw::*;
use opendal_core::*;

use asyncband::mutex::Mutex;
use log::debug;

use super::GDRIVE_SCHEME;
use super::config::GdriveConfig;
use super::core::GdrivePathQuery;
use super::core::GdriveSigner;
use super::path_index::GdrivePathIndex;

/// [GoogleDrive](https://drive.google.com/) backend support.
#[derive(Default)]
#[doc = include_str!("docs.md")]
pub struct GdriveBuilder {
    pub(super) config: GdriveConfig,
}

impl Debug for GdriveBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GdriveBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl GdriveBuilder {
    /// Set root path of GoogleDrive folder.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set the shared drive (formerly Team Drive) id of GoogleDrive.
    ///
    /// When set, `root` resolves inside this shared drive instead of the
    /// user's My Drive. The id appears at the end of the shared drive URL,
    /// for example `https://drive.google.com/drive/folders/<drive_id>`.
    pub fn drive_id(mut self, drive_id: &str) -> Self {
        self.config.drive_id = if drive_id.is_empty() {
            None
        } else {
            Some(drive_id.to_string())
        };

        self
    }

    /// Access token is used for temporary access to the GoogleDrive API.
    ///
    /// You can get the access token from [GoogleDrive App Console](https://console.cloud.google.com/apis/credentials)
    /// or [GoogleDrive OAuth2 Playground](https://developers.google.com/oauthplayground/)
    ///
    /// # Note
    ///
    /// - An access token is valid for 1 hour.
    /// - If you want to use the access token for a long time,
    ///   you can use the refresh token to get a new access token.
    pub fn access_token(mut self, access_token: &str) -> Self {
        self.config.access_token = Some(access_token.to_string());
        self
    }

    /// Refresh token is used for long term access to the GoogleDrive API.
    ///
    /// You can get the refresh token via OAuth 2.0 Flow of GoogleDrive API.
    ///
    /// OpenDAL will use this refresh token to get a new access token when the old one is expired.
    pub fn refresh_token(mut self, refresh_token: &str) -> Self {
        self.config.refresh_token = Some(refresh_token.to_string());
        self
    }

    /// Set the client id for GoogleDrive.
    ///
    /// This is required for OAuth 2.0 Flow to refresh the access token.
    pub fn client_id(mut self, client_id: &str) -> Self {
        self.config.client_id = Some(client_id.to_string());
        self
    }

    /// Set the client secret for GoogleDrive.
    ///
    /// This is required for OAuth 2.0 Flow with refresh the access token.
    pub fn client_secret(mut self, client_secret: &str) -> Self {
        self.config.client_secret = Some(client_secret.to_string());
        self
    }
}

impl Builder for GdriveBuilder {
    type Config = GdriveConfig;

    fn build(self) -> Result<impl Service> {
        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let info = ServiceInfo::new(GDRIVE_SCHEME, &root, "");
        let capability = Capability {
            stat: true,

            read: true,
            read_with_suffix: true,

            list: true,
            list_with_recursive: true,

            write: true,

            create_dir: true,
            delete: true,
            delete_with_recursive: true,
            rename: true,
            copy: true,

            shared: true,

            ..Default::default()
        };

        let accessor_info = info;
        let mut signer = GdriveSigner::new();
        match (self.config.access_token, self.config.refresh_token) {
            (Some(access_token), None) => {
                signer.access_token = access_token;
                // We will never expire user specified access token.
                signer.expires_in = Timestamp::MAX;
            }
            (None, Some(refresh_token)) => {
                let client_id = self.config.client_id.ok_or_else(|| {
                    Error::new(
                        ErrorKind::ConfigInvalid,
                        "client_id must be set when refresh_token is set",
                    )
                    .with_context("service", GDRIVE_SCHEME)
                })?;
                let client_secret = self.config.client_secret.ok_or_else(|| {
                    Error::new(
                        ErrorKind::ConfigInvalid,
                        "client_secret must be set when refresh_token is set",
                    )
                    .with_context("service", GDRIVE_SCHEME)
                })?;

                signer.refresh_token = refresh_token;
                signer.client_id = client_id;
                signer.client_secret = client_secret;
            }
            (Some(_), Some(_)) => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "access_token and refresh_token cannot be set at the same time",
                )
                .with_context("service", GDRIVE_SCHEME));
            }
            (None, None) => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "access_token or refresh_token must be set",
                )
                .with_context("service", GDRIVE_SCHEME));
            }
        };

        let signer = Arc::new(Mutex::new(signer));
        let drive_id = self.config.drive_id.filter(|v| !v.is_empty());

        Ok(GdriveBackend {
            core: Arc::new(GdriveCore {
                info: accessor_info.clone(),
                capability,
                root,
                drive_id: drive_id.clone(),
                signer: signer.clone(),
                path_index: GdrivePathIndex::new(GdrivePathQuery::new(signer, drive_id)),
                recent_entries: Mutex::default(),
            }),
        })
    }
}

#[derive(Clone, Debug)]
pub struct GdriveBackend {
    pub core: Arc<GdriveCore>,
}

/// Lister type that supports both recursive and non-recursive listing
pub type GdriveListers = TwoWays<oio::PageLister<GdriveLister>, GdriveFlatLister>;

impl Service for GdriveBackend {
    type Reader = oio::StreamReader<GdriveReader>;
    type Writer = oio::OneShotWriter<GdriveWriter>;
    type Lister = GdriveListers;
    type Deleter = oio::OneShotDeleter<GdriveDeleter>;
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
        let path = build_abs_path(&self.core.root, path);
        let dir_id = self.core.ensure_dir(ctx, &path).await?;
        let metadata = MetadataBuilder::dir().build();

        self.core.cache_dir_id(&path, &dir_id).await;
        self.core.record_recent_upsert(&path, metadata).await;

        Ok(RpCreateDir::default())
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let path = build_abs_path(&self.core.root, path);

        match self.core.recent_entry_for_path(&path).await {
            GdriveRecentPathState::Present(metadata) => {
                if metadata.mode().is_dir() && path.ends_with('/') {
                    return Ok(RpStat::new(*metadata));
                }
            }
            GdriveRecentPathState::Deleted => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    format!("path not found: {path}"),
                ));
            }
            GdriveRecentPathState::Missing => {}
        }

        let mut file_id = match self.core.resolve_path(ctx, &path).await? {
            Some(id) => id,
            None => match self.core.resolve_path_after_refresh(ctx, &path).await? {
                Some(id) => id,
                None => {
                    return Err(Error::new(
                        ErrorKind::NotFound,
                        format!("path not found: {path}"),
                    ));
                }
            },
        };
        let mut resp = self.core.gdrive_stat_by_id(ctx, &file_id).await?;

        if resp.status() == StatusCode::NOT_FOUND {
            file_id = self
                .core
                .resolve_path_after_refresh(ctx, &path)
                .await?
                .ok_or(Error::new(
                    ErrorKind::NotFound,
                    format!("path not found: {path}"),
                ))?;
            resp = self.core.gdrive_stat_by_id(ctx, &file_id).await?;
        }

        if resp.status() != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetFile")),
                resp,
            ));
        }

        let bs = resp.into_body();
        let gdrive_file: GdriveFile =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        let file_type = if gdrive_file.mime_type == "application/vnd.google-apps.folder" {
            EntryMode::DIR
        } else {
            EntryMode::FILE
        };
        let mut meta = if file_type == EntryMode::FILE {
            let size = gdrive_file.size.as_deref().ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "gdrive stat response does not contain file size",
                )
            })?;
            MetadataBuilder::file(size.parse::<u64>().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "parse content length").set_source(e)
            })?)
        } else {
            MetadataBuilder::dir()
        };
        meta.content_type(gdrive_file.mime_type);
        if let Some(v) = gdrive_file.modified_time {
            meta.last_modified(v.parse::<Timestamp>().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "parse last modified time").set_source(e)
            })?);
        }
        Ok(RpStat::new(meta.build()))
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<GdriveReader> = {
            Ok(oio::StreamReader::new(GdriveReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: oio::OneShotWriter<GdriveWriter> = {
            let path = build_abs_path(&self.core.root, path);

            Ok(oio::OneShotWriter::new(GdriveWriter::new(
                self.core.clone(),
                ctx.clone(),
                path,
                None,
            )))
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<GdriveDeleter> = {
            Ok(oio::OneShotDeleter::new(GdriveDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: GdriveListers = {
            let path = build_abs_path(&self.core.root, path);

            if args.recursive() {
                // Use optimized batch-query recursive lister
                let l = GdriveFlatLister::new(path, self.core.clone(), ctx.clone());
                Ok(TwoWays::Two(l))
            } else {
                // Use standard page-based lister for non-recursive
                let l = GdriveLister::new(path, self.core.clone(), ctx.clone());
                Ok(TwoWays::One(oio::PageLister::new(l)))
            }
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _args: OpCopy,
    ) -> Result<Self::Copier> {
        let core = self.core.clone();
        let ctx = ctx.clone();
        let from = from.to_string();
        let to = to.to_string();

        Ok(oio::OneShotCopier::new(async move {
            let source = build_abs_path(&core.root, &from);
            let target = build_abs_path(&core.root, &to);
            let resp = core.gdrive_copy(&ctx, &from, &to).await?;

            match resp.status() {
                StatusCode::OK => {
                    let body = resp.into_body();
                    let meta: GdriveFile = serde_json::from_reader(body.reader())
                        .map_err(new_json_deserialize_error)?;

                    let to_path = build_abs_path(&core.root, &to);
                    let is_dir = meta.mime_type == "application/vnd.google-apps.folder";
                    let metadata = if is_dir {
                        MetadataBuilder::dir()
                    } else {
                        let size = meta.size.as_deref().ok_or_else(|| {
                            Error::new(
                                ErrorKind::Unexpected,
                                "gdrive copy response does not contain file size",
                            )
                        })?;
                        MetadataBuilder::file(size.parse::<u64>().map_err(|e| {
                            Error::new(ErrorKind::Unexpected, "parse content length").set_source(e)
                        })?)
                    };

                    if is_dir {
                        core.cache_dir_id(&to_path, &meta.id).await;
                    } else {
                        core.cache_file_id(&to_path, &meta.id).await;
                    }
                    let metadata = metadata.build();
                    core.record_recent_upsert(&to_path, metadata.clone()).await;

                    Ok(metadata)
                }
                StatusCode::NOT_FOUND => {
                    core.refresh_path(&source).await;
                    core.refresh_path(&target).await;
                    let resp = core.gdrive_copy(&ctx, &from, &to).await?;
                    match resp.status() {
                        StatusCode::OK => {
                            let body = resp.into_body();
                            let meta: GdriveFile = serde_json::from_reader(body.reader())
                                .map_err(new_json_deserialize_error)?;

                            let to_path = build_abs_path(&core.root, &to);
                            let is_dir = meta.mime_type == "application/vnd.google-apps.folder";
                            let metadata = if is_dir {
                                MetadataBuilder::dir()
                            } else {
                                let size = meta.size.as_deref().ok_or_else(|| {
                                    Error::new(
                                        ErrorKind::Unexpected,
                                        "gdrive copy response does not contain file size",
                                    )
                                })?;
                                MetadataBuilder::file(size.parse::<u64>().map_err(|e| {
                                    Error::new(ErrorKind::Unexpected, "parse content length")
                                        .set_source(e)
                                })?)
                            };

                            if is_dir {
                                core.cache_dir_id(&to_path, &meta.id).await;
                            } else {
                                core.cache_file_id(&to_path, &meta.id).await;
                            }
                            let metadata = metadata.build();
                            core.record_recent_upsert(&to_path, metadata.clone()).await;

                            Ok(metadata)
                        }
                        _ => Err(parse_error(
                            ErrorContext::new(ServiceOperation("CopyFile")),
                            resp,
                        )),
                    }
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
        _args: OpRename,
    ) -> Result<RpRename> {
        let source = build_abs_path(&self.core.root, from);
        let target = build_abs_path(&self.core.root, to);

        // rename will overwrite `to`, delete it if exist
        self.core.trash_path_if_exists(ctx, &target).await?;

        let resp = self
            .core
            .gdrive_patch_metadata_request(ctx, &source, &target)
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let body = resp.into_body();
                let meta: GdriveFile =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

                let source_path = if meta.mime_type == "application/vnd.google-apps.folder" {
                    normalize_dir_path(&build_abs_path(&self.core.root, from))
                } else {
                    build_abs_path(&self.core.root, from)
                };
                let target_path = if meta.mime_type == "application/vnd.google-apps.folder" {
                    normalize_dir_path(&build_abs_path(&self.core.root, to))
                } else {
                    build_abs_path(&self.core.root, to)
                };
                let is_dir = meta.mime_type == "application/vnd.google-apps.folder";
                let metadata = if is_dir {
                    MetadataBuilder::dir()
                } else {
                    let size = meta.size.as_deref().ok_or_else(|| {
                        Error::new(
                            ErrorKind::Unexpected,
                            "gdrive move response does not contain file size",
                        )
                    })?;
                    MetadataBuilder::file(size.parse::<u64>().map_err(|e| {
                        Error::new(ErrorKind::Unexpected, "parse content length").set_source(e)
                    })?)
                };

                if is_dir {
                    self.core.invalidate_dir_id(&source_path).await;
                    self.core.cache_dir_id(&target_path, &meta.id).await;
                } else {
                    self.core.invalidate_file_id(&source_path).await;
                    self.core.cache_file_id(&target_path, &meta.id).await;
                }
                self.core
                    .record_recent_delete(
                        &source_path,
                        if is_dir {
                            EntryMode::DIR
                        } else {
                            EntryMode::FILE
                        },
                    )
                    .await;
                self.core
                    .record_recent_upsert(&target_path, metadata.build())
                    .await;

                Ok(RpRename::default())
            }
            StatusCode::NOT_FOUND => {
                self.core.refresh_path(&source).await;
                self.core.refresh_path(&target).await;

                let resp = self
                    .core
                    .gdrive_patch_metadata_request(ctx, &source, &target)
                    .await?;

                if resp.status() != StatusCode::OK {
                    return Err(parse_error(
                        ErrorContext::new(ServiceOperation("MoveFile")),
                        resp,
                    ));
                }

                let body = resp.into_body();
                let meta: GdriveFile =
                    serde_json::from_reader(body.reader()).map_err(new_json_deserialize_error)?;

                let source_path = if meta.mime_type == "application/vnd.google-apps.folder" {
                    normalize_dir_path(&build_abs_path(&self.core.root, from))
                } else {
                    build_abs_path(&self.core.root, from)
                };
                let target_path = if meta.mime_type == "application/vnd.google-apps.folder" {
                    normalize_dir_path(&build_abs_path(&self.core.root, to))
                } else {
                    build_abs_path(&self.core.root, to)
                };
                let is_dir = meta.mime_type == "application/vnd.google-apps.folder";
                let metadata = if is_dir {
                    MetadataBuilder::dir()
                } else {
                    let size = meta.size.as_deref().ok_or_else(|| {
                        Error::new(
                            ErrorKind::Unexpected,
                            "gdrive move response does not contain file size",
                        )
                    })?;
                    MetadataBuilder::file(size.parse::<u64>().map_err(|e| {
                        Error::new(ErrorKind::Unexpected, "parse content length").set_source(e)
                    })?)
                };

                if is_dir {
                    self.core.invalidate_dir_id(&source_path).await;
                    self.core.cache_dir_id(&target_path, &meta.id).await;
                } else {
                    self.core.invalidate_file_id(&source_path).await;
                    self.core.cache_file_id(&target_path, &meta.id).await;
                }
                self.core
                    .record_recent_delete(
                        &source_path,
                        if is_dir {
                            EntryMode::DIR
                        } else {
                            EntryMode::FILE
                        },
                    )
                    .await;
                self.core
                    .record_recent_upsert(&target_path, metadata.build())
                    .await;

                Ok(RpRename::default())
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("MoveFile")),
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
