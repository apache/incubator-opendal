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

use opendal_core::raw::*;
use opendal_core::*;
use smb::ClientConfig;
use smb::ConnectionConfig;
use smb::FileAccessMask;
use smb::FileAllInformation;
use smb::FileCreateArgs;
use smb::FileRenameInformation;
use smb::Resource;
use smb::UncPath;
use tokio::sync::OnceCell;
use url::Url;

use super::SMB_SCHEME;
use super::config::SmbConfig;
use super::copier::SmbCopier;
use super::core::RuntimeResource;
use super::core::SmbCore;
use super::core::to_metadata;
use super::deleter::SmbDeleter;
use super::error::parse_smb_error;
use super::lister::SmbLister;
use super::reader::SmbReader;
use super::writer::SmbWriter;

/// Access files and directories on an SMB2 or SMB3 share.
///
/// The service uses `smb-rs` with asynchronous TCP transport, NTLM authentication,
/// signing, and encryption. Operations require a Tokio runtime.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct SmbBuilder {
    pub(super) config: SmbConfig,
}

impl SmbBuilder {
    /// Set the server hostname or IP address, optionally followed by a port.
    ///
    /// The default port is 445. Use `[::1]:445` for an IPv6 address with a port.
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.config.endpoint = endpoint.to_string();
        self
    }

    /// Set the SMB share name.
    pub fn share(mut self, share: &str) -> Self {
        self.config.share = share.to_string();
        self
    }

    /// Set the root directory within the share. Defaults to `/`.
    ///
    /// Directory creation and writes create missing parent directories.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = Some(root.to_string());
        self
    }

    /// Set the NTLM username.
    ///
    /// Use `DOMAIN\user` or `user@domain` for a domain-qualified username.
    pub fn user(mut self, user: &str) -> Self {
        self.config.user = Some(user.to_string());
        self
    }

    /// Set the password. An empty password is allowed.
    pub fn password(mut self, password: &str) -> Self {
        self.config.password = Some(password.to_string());
        self
    }
}

impl Builder for SmbBuilder {
    type Config = SmbConfig;

    fn build(self) -> Result<impl Service> {
        let endpoint = self.config.endpoint.as_str();
        if endpoint.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is required"));
        }
        let url = Url::parse(&format!("smb://{endpoint}")).map_err(|e| {
            Error::new(ErrorKind::ConfigInvalid, "invalid SMB endpoint").set_source(e)
        })?;
        if url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || !matches!(url.path(), "" | "/")
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "endpoint must contain only a hostname or IP address and optional port",
            ));
        }
        let share = self.config.share.as_str();
        if share.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "share is required"));
        }
        let share_path = UncPath::new(url.host_str().expect("host was checked"))
            .and_then(|p| p.with_share(share))
            .map_err(|e| Error::new(ErrorKind::ConfigInvalid, "invalid SMB share").set_source(e))?;
        let username = self.config.user.unwrap_or_default();
        let root = normalize_root(self.config.root.as_deref().unwrap_or("/"));
        let client_config = ClientConfig {
            dfs: false,
            connection: ConnectionConfig {
                port: url.port(),
                ..Default::default()
            },
            ..Default::default()
        };

        let core = Arc::new(SmbCore {
            info: ServiceInfo::new(SMB_SCHEME, &root, share),
            root,
            share: share_path,
            username,
            password: self.config.password.unwrap_or_default(),
            client_config,
            client: OnceCell::new(),
        });
        core.path("/")?;
        Ok(SmbBackend { core })
    }
}

#[derive(Clone, Debug)]
pub(super) struct SmbBackend {
    core: Arc<SmbCore>,
}

impl Service for SmbBackend {
    type Reader = oio::PositionReader<SmbReader>;
    type Writer = SmbWriter;
    type Lister = SmbLister;
    type Deleter = oio::OneShotDeleter<SmbDeleter>;
    type Copier = oio::OneShotCopier;
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info.clone()
    }

    fn capability(&self) -> Capability {
        Capability {
            stat: true,
            read: true,
            write: true,
            write_can_empty: true,
            write_can_multi: true,
            write_with_if_not_exists: true,
            create_dir: true,
            delete: true,
            list: true,
            copy: true,
            copy_with_if_not_exists: true,
            rename: true,
            rename_with_if_not_exists: true,
            shared: true,
            ..Default::default()
        }
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.core.create_dir(path).await?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let target = self.core.path(path)?;
        let client = self.core.client().await?;
        let args = FileCreateArgs::make_open_existing(
            FileAccessMask::new().with_file_read_attributes(true),
        );
        let resource = client
            .create_file(&target, &args)
            .await
            .map_err(parse_smb_error)?;
        let info = match resource {
            Resource::File(file) => {
                let file = RuntimeResource::new(file);
                let info = file.query_info::<FileAllInformation>().await;
                file.close().await.map_err(parse_smb_error)?;
                info.map_err(parse_smb_error)?
            }
            Resource::Directory(dir) => {
                let dir = RuntimeResource::new(dir);
                let info = dir.query_info::<FileAllInformation>().await;
                dir.close().await.map_err(parse_smb_error)?;
                info.map_err(parse_smb_error)?
            }
            Resource::Pipe(_) => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "SMB pipes are not supported",
                ));
            }
        };
        let metadata = to_metadata(
            info.basic.file_attributes,
            info.standard.end_of_file,
            info.basic.last_write_time,
        )?;
        Ok(RpStat::new(metadata))
    }

    fn read(&self, _ctx: &OperationContext, path: &str, _: OpRead) -> Result<Self::Reader> {
        self.core.path(path)?;
        Ok(oio::PositionReader::new(SmbReader::new(
            self.core.clone(),
            path,
        )))
    }

    fn write(&self, _ctx: &OperationContext, path: &str, op: OpWrite) -> Result<Self::Writer> {
        self.core.path(path)?;
        Ok(SmbWriter::new(self.core.clone(), path, op))
    }

    fn list(&self, _ctx: &OperationContext, path: &str, _: OpList) -> Result<Self::Lister> {
        self.core.path(path)?;
        Ok(SmbLister::new(self.core.clone(), path))
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        Ok(oio::OneShotDeleter::new(SmbDeleter::new(self.core.clone())))
    }

    fn copy(
        &self,
        _ctx: &OperationContext,
        from: &str,
        to: &str,
        op: OpCopy,
    ) -> Result<Self::Copier> {
        let source = self.core.path(from)?;
        let target = self.core.path(to)?;
        let parent = to.rsplit_once('/').map(|(parent, _)| parent.to_string());
        let core = self.core.clone();
        Ok(oio::OneShotCopier::new(async move {
            SmbCopier::copy(core, source, target, parent, op).await
        }))
    }

    async fn rename(
        &self,
        _ctx: &OperationContext,
        from: &str,
        to: &str,
        op: OpRename,
    ) -> Result<RpRename> {
        let source = self.core.path(from)?;
        let target = self.core.path(to)?;
        if let Some((parent, _)) = to.rsplit_once('/') {
            self.core.create_dir(parent).await?;
        }
        let client = self.core.client().await?;
        let args = FileCreateArgs::make_open_existing(FileAccessMask::new().with_delete(true));
        let resource = client
            .create_file(&source, &args)
            .await
            .map_err(parse_smb_error)?;
        let file = match resource {
            Resource::File(file) => RuntimeResource::new(file),
            Resource::Directory(dir) => {
                RuntimeResource::new(dir)
                    .close()
                    .await
                    .map_err(parse_smb_error)?;
                return Err(Error::new(
                    ErrorKind::IsADirectory,
                    "cannot rename a directory",
                ));
            }
            Resource::Pipe(_) => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "SMB pipes are not supported",
                ));
            }
        };
        let result = file
            .set_info(FileRenameInformation {
                replace_if_exists: (!op.if_not_exists()).into(),
                root_directory: 0,
                file_name: target.path().unwrap_or_default().into(),
            })
            .await;
        file.close().await.map_err(parse_smb_error)?;
        result.map_err(|error| {
            let error = parse_smb_error(error);
            if op.if_not_exists() && error.kind() == ErrorKind::AlreadyExists {
                Error::new(ErrorKind::ConditionNotMatch, "file already exists").set_source(error)
            } else {
                error
            }
        })?;
        Ok(RpRename::default())
    }

    async fn presign(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _: OpPresign,
    ) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "SMB presign is not supported",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_rejects_invalid_connection_config() {
        for builder in [
            SmbBuilder::default(),
            SmbBuilder::default().endpoint("host").user("user"),
            SmbBuilder::default()
                .endpoint("host/path")
                .share("data")
                .user("user"),
            SmbBuilder::default()
                .endpoint("host")
                .share("data/path")
                .user("user"),
            SmbBuilder::default()
                .endpoint("host")
                .share("data")
                .user("user")
                .root("../outside"),
        ] {
            let Err(error) = builder.build() else {
                panic!("invalid configuration was accepted")
            };
            assert_eq!(error.kind(), ErrorKind::ConfigInvalid);
        }
    }

    #[test]
    fn builder_normalizes_root_without_connecting() {
        let backend = SmbBuilder::default()
            .endpoint("localhost:1445")
            .share("data")
            .user("user")
            .root("//documents///reports/")
            .build()
            .unwrap();
        assert_eq!(backend.info().root().as_ref(), "/documents/reports/");
    }
}
