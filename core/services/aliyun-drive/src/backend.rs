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

use asyncband::mutex::Mutex;
use bytes::Buf;
use log::debug;

use super::ALIYUN_DRIVE_SCHEME;
use super::config::AliyunDriveConfig;
use super::core::*;
use super::deleter::AliyunDriveDeleter;
use super::lister::AliyunDriveLister;
use super::reader::*;
use super::writer::AliyunDriveLazyWriter;
use opendal_core::raw::*;
use opendal_core::*;

#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct AliyunDriveBuilder {
    pub(super) config: AliyunDriveConfig,
}

impl Debug for AliyunDriveBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AliyunDriveBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AliyunDriveBuilder {
    /// Set the root of this backend.
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

    /// Set access_token of this backend.
    pub fn access_token(mut self, access_token: &str) -> Self {
        self.config.access_token = Some(access_token.to_string());

        self
    }

    /// Set client_id of this backend.
    pub fn client_id(mut self, client_id: &str) -> Self {
        self.config.client_id = Some(client_id.to_string());

        self
    }

    /// Set client_secret of this backend.
    pub fn client_secret(mut self, client_secret: &str) -> Self {
        self.config.client_secret = Some(client_secret.to_string());

        self
    }

    /// Set refresh_token of this backend.
    pub fn refresh_token(mut self, refresh_token: &str) -> Self {
        self.config.refresh_token = Some(refresh_token.to_string());

        self
    }

    /// Set drive_type of this backend.
    pub fn drive_type(mut self, drive_type: &str) -> Self {
        self.config.drive_type = drive_type.to_string();

        self
    }
}

impl Builder for AliyunDriveBuilder {
    type Config = AliyunDriveConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", root);

        let sign = match self.config.access_token.clone() {
            Some(access_token) if !access_token.is_empty() => {
                AliyunDriveSign::Access(access_token)
            }
            _ => match (
                self.config.client_id.clone(),
                self.config.client_secret.clone(),
                self.config.refresh_token.clone(),
            ) {
                (Some(client_id), Some(client_secret), Some(refresh_token)) if
                !client_id.is_empty() && !client_secret.is_empty() && !refresh_token.is_empty() => {
                    AliyunDriveSign::Refresh(client_id, client_secret, refresh_token, None, 0)
                }
                _ => return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "access_token and a set of client_id, client_secret, and refresh_token are both missing.")
                    .with_operation("Builder::build")
                    .with_context("service", ALIYUN_DRIVE_SCHEME)),
            },
        };

        let drive_type = match self.config.drive_type.as_str() {
            "" | "default" => DriveType::Default,
            "resource" => DriveType::Resource,
            "backup" => DriveType::Backup,
            _ => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "drive_type is invalid.",
                ));
            }
        };
        debug!("backend use drive_type {drive_type:?}");

        Ok(AliyunDriveBackend {
            core: Arc::new(AliyunDriveCore {
                info: ServiceInfo::new(ALIYUN_DRIVE_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,
                    create_dir: true,
                    read: true,
                    read_with_suffix: true,
                    write: true,
                    write_can_multi: true,
                    // The min multipart size of AliyunDrive is 100 KiB.
                    write_multi_min_size: Some(100 * 1024),
                    // The max multipart size of AliyunDrive is 5 GiB.
                    write_multi_max_size: if cfg!(target_pointer_width = "64") {
                        Some(5 * 1024 * 1024 * 1024)
                    } else {
                        Some(usize::MAX)
                    },
                    delete: true,
                    copy: true,
                    rename: true,
                    list: true,
                    list_with_limit: true,
                    shared: true,
                    ..Default::default()
                },
                endpoint: "https://openapi.alipan.com".to_string(),
                root,
                drive_type,
                signer: Arc::new(Mutex::new(AliyunDriveSigner {
                    drive_id: None,
                    sign,
                })),
                dir_lock: Arc::new(Mutex::new(())),
            }),
        })
    }
}

#[derive(Clone, Debug)]
pub struct AliyunDriveBackend {
    pub(crate) core: Arc<AliyunDriveCore>,
}

impl Service for AliyunDriveBackend {
    type Reader = oio::StreamReader<AliyunDriveReader>;
    type Writer = AliyunDriveLazyWriter;
    type Lister = oio::PageLister<AliyunDriveLister>;
    type Deleter = oio::OneShotDeleter<AliyunDriveDeleter>;
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
        self.core.ensure_dir_exists(ctx, path).await?;

        Ok(RpCreateDir::default())
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
        let res = self.core.get_by_path(ctx, from).await?;
        let file: AliyunDriveFile =
            serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;
        // rename can overwrite.
        match self.core.get_by_path(ctx, to).await {
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err),
            Ok(res) => {
                let file: AliyunDriveFile =
                    serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;
                self.core.delete_path(ctx, &file.file_id).await?;
            }
        };

        let parent_file_id = self.core.ensure_dir_exists(ctx, get_parent(to)).await?;
        self.core
            .move_path(ctx, &file.file_id, &parent_file_id)
            .await?;

        let from_name = get_basename(from);
        let to_name = get_basename(to);

        if from_name != to_name {
            self.core.update_path(ctx, &file.file_id, to_name).await?;
        }

        Ok(RpRename::default())
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
        let source_content_length_hint = args.source_content_length_hint();

        Ok(oio::OneShotCopier::new(async move {
            let res = core.get_by_path(&ctx, &from).await?;
            let file: AliyunDriveFile =
                serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;
            let source_content_length =
                file.size.or(source_content_length_hint).ok_or_else(|| {
                    Error::new(
                        ErrorKind::Unexpected,
                        "Aliyun Drive source file does not contain a size",
                    )
                })?;

            if from == to {
                Ok(MetadataBuilder::file(source_content_length).build())
            } else {
                // copy can overwrite.
                match core.get_by_path(&ctx, &to).await {
                    Err(err) if err.kind() == ErrorKind::NotFound => {}
                    Err(err) => Err(err)?,
                    Ok(res) => {
                        let file: AliyunDriveFile = serde_json::from_reader(res.reader())
                            .map_err(new_json_serialize_error)?;
                        core.delete_path(&ctx, &file.file_id).await?;
                    }
                }
                // there is no direct copy in AliyunDrive.
                // so we need to copy the path first and then rename it.
                let parent_path = get_parent(&to);
                let parent_file_id = core.ensure_dir_exists(&ctx, parent_path).await?;

                // if from and to are going to be placed in the same folder,
                // copy_path will fail as we cannot change the name during this action.
                // it has to be auto renamed.
                let auto_rename = file.parent_file_id == parent_file_id;
                let res = core
                    .copy_path(&ctx, &file.file_id, &parent_file_id, auto_rename)
                    .await?;
                let file: CopyResponse =
                    serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;
                let file_id = file.file_id;

                let from_name = get_basename(&from);
                let to_name = get_basename(&to);

                if from_name != to_name {
                    core.update_path(&ctx, &file_id, to_name).await?;
                }

                Ok(MetadataBuilder::file(source_content_length).build())
            }
        }))
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let res = self.core.get_by_path(ctx, path).await?;
        let file: AliyunDriveFile =
            serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;

        if file.path_type == "folder" {
            let mut meta = MetadataBuilder::dir();
            meta.last_modified(file.updated_at.parse::<Timestamp>().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "parse last modified time").set_source(e)
            })?);

            return Ok(RpStat::new(meta.build()));
        }

        let size = file.size.ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "aliyun drive stat response does not contain file size",
            )
        })?;
        let mut meta = MetadataBuilder::file(size);
        meta.last_modified(file.updated_at.parse::<Timestamp>().map_err(|e| {
            Error::new(ErrorKind::Unexpected, "parse last modified time").set_source(e)
        })?);
        if let Some(v) = file.content_type {
            meta.content_type(v);
        }

        Ok(RpStat::new(meta.build()))
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<AliyunDriveReader> = {
            Ok(oio::StreamReader::new(AliyunDriveReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<AliyunDriveDeleter> = {
            Ok(oio::OneShotDeleter::new(AliyunDriveDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<AliyunDriveLister> = {
            let l = AliyunDriveLister::new_with_path(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
                args.limit(),
            );

            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        Ok(AliyunDriveLazyWriter::new(
            self.core.clone(),
            ctx.clone(),
            path.to_string(),
            args,
        ))
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
