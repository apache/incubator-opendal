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
use std::path::Path;
use std::path::PathBuf;

use async_trait::async_trait;
use log::debug;
use openssh::KnownHosts;
use openssh::SessionBuilder;
use openssh_sftp_client::Sftp;
use openssh_sftp_client::SftpOptions;
use serde::Deserialize;

use super::error::is_not_found;
use super::error::is_sftp_protocol_error;
use super::error::parse_sftp_error;
use super::error::parse_ssh_error;
use super::lister::SftpLister;
use super::reader::SftpReader;
use super::writer::SftpWriter;
use crate::raw::*;
use crate::*;

/// Config for Sftpservices support.
#[derive(Default, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct SftpConfig {
    /// endpoint of this backend
    pub endpoint: Option<String>,
    /// root of this backend
    pub root: Option<String>,
    /// user of this backend
    pub user: Option<String>,
    /// key of this backend
    pub key: Option<String>,
    /// known_hosts_strategy of this backend
    pub known_hosts_strategy: Option<String>,
    /// enable_copy of this backend
    pub enable_copy: bool,
}

impl Debug for SftpConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SftpConfig")
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

/// SFTP services support. (only works on unix)
///
/// If you are interested in working on windows, pl ease refer to [this](https://github.com/apache/opendal/issues/2963) issue.
/// Welcome to leave your comments or make contributions.
///
/// Warning: Maximum number of file holdings is depending on the remote system configuration.
///
/// For example, the default value is 255 in macOS, and 1024 in linux. If you want to open
/// lots of files, you should pay attention to close the file after using it.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct SftpBuilder {
    config: SftpConfig,
}

impl Debug for SftpBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SftpBuilder")
            .field("config", &self.config)
            .finish()
    }
}

impl SftpBuilder {
    /// set endpoint for sftp backend.
    /// The format is same as `openssh`, using either `[user@]hostname` or `ssh://[user@]hostname[:port]`. A username or port that is specified in the endpoint overrides the one set in the builder (but does not change the builder).
    pub fn endpoint(&mut self, endpoint: &str) -> &mut Self {
        self.config.endpoint = if endpoint.is_empty() {
            None
        } else {
            Some(endpoint.to_string())
        };

        self
    }

    /// set root path for sftp backend.
    /// It uses the default directory set by the remote `sftp-server` as default.
    pub fn root(&mut self, root: &str) -> &mut Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// set user for sftp backend.
    pub fn user(&mut self, user: &str) -> &mut Self {
        self.config.user = if user.is_empty() {
            None
        } else {
            Some(user.to_string())
        };

        self
    }

    /// set key path for sftp backend.
    pub fn key(&mut self, key: &str) -> &mut Self {
        self.config.key = if key.is_empty() {
            None
        } else {
            Some(key.to_string())
        };

        self
    }

    /// set known_hosts strategy for sftp backend.
    /// available values:
    /// - Strict (default)
    /// - Accept
    /// - Add
    pub fn known_hosts_strategy(&mut self, strategy: &str) -> &mut Self {
        self.config.known_hosts_strategy = if strategy.is_empty() {
            None
        } else {
            Some(strategy.to_string())
        };

        self
    }

    /// set enable_copy for sftp backend.
    /// It requires the server supports copy-file extension.
    pub fn enable_copy(&mut self, enable_copy: bool) -> &mut Self {
        self.config.enable_copy = enable_copy;

        self
    }
}

impl Builder for SftpBuilder {
    const SCHEME: Scheme = Scheme::Sftp;
    type Accessor = SftpBackend;

    fn build(&mut self) -> Result<Self::Accessor> {
        debug!("sftp backend build started: {:?}", &self);
        let endpoint = match self.config.endpoint.clone() {
            Some(v) => v,
            None => return Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")),
        };

        let user = self.config.user.clone();

        let root = self
            .config
            .root
            .clone()
            .map(|r| normalize_root(r.as_str()))
            .unwrap_or_default();

        let known_hosts_strategy = match &self.config.known_hosts_strategy {
            Some(v) => {
                let v = v.to_lowercase();
                if v == "strict" {
                    KnownHosts::Strict
                } else if v == "accept" {
                    KnownHosts::Accept
                } else if v == "add" {
                    KnownHosts::Add
                } else {
                    return Err(Error::new(
                        ErrorKind::ConfigInvalid,
                        format!("unknown known_hosts strategy: {}", v).as_str(),
                    ));
                }
            }
            None => KnownHosts::Strict,
        };

        debug!("sftp backend finished: {:?}", &self);

        Ok(SftpBackend {
            endpoint,
            root,
            user,
            key: self.config.key.clone(),
            known_hosts_strategy,
            copyable: self.config.enable_copy,
        })
    }

    fn from_map(map: HashMap<String, String>) -> Self {
        SftpBuilder {
            config: SftpConfig::deserialize(ConfigDeserializer::new(map))
                .expect("config deserialize must succeed"),
        }
    }
}

/// Backend is used to serve `Accessor` support for sftp.
#[derive(Clone)]
pub struct SftpBackend {
    endpoint: String,
    root: String,
    user: Option<String>,
    key: Option<String>,
    known_hosts_strategy: KnownHosts,
    copyable: bool,
}

impl Debug for SftpBackend {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backend").finish()
    }
}

#[async_trait]
impl Accessor for SftpBackend {
    type Reader = SftpReader;
    type Writer = SftpWriter;
    type Lister = Option<SftpLister>;
    type BlockingReader = ();
    type BlockingWriter = ();
    type BlockingLister = ();

    fn info(&self) -> AccessorInfo {
        let mut am = AccessorInfo::default();
        am.set_root(self.root.as_str())
            .set_scheme(Scheme::Sftp)
            .set_native_capability(Capability {
                stat: true,

                read: true,

                write: true,
                write_can_multi: true,

                create_dir: true,
                delete: true,

                list: true,
                list_with_limit: true,

                copy: self.copyable,
                rename: true,

                ..Default::default()
            });

        am
    }

    async fn create_dir(&self, path: &str, _: OpCreateDir) -> Result<RpCreateDir> {
        let client = self.connect().await?;
        let mut fs = client.fs();
        fs.set_cwd(&self.root);

        let paths = Path::new(&path).components();
        let mut current = PathBuf::from(&self.root);
        for p in paths {
            current = current.join(p);
            let res = fs.create_dir(p).await;

            if let Err(e) = res {
                // ignore error if dir already exists
                if !is_sftp_protocol_error(&e) {
                    return Err(parse_sftp_error(e));
                }
            }
            fs.set_cwd(&current);
        }

        return Ok(RpCreateDir::default());
    }

    async fn stat(&self, path: &str, _: OpStat) -> Result<RpStat> {
        let client = self.connect().await?;
        let mut fs = client.fs();
        fs.set_cwd(&self.root);

        let meta: Metadata = fs.metadata(path).await.map_err(parse_sftp_error)?.into();

        Ok(RpStat::new(meta))
    }

    async fn read(&self, path: &str, _: OpRead) -> Result<(RpRead, Self::Reader)> {
        Ok((
            RpRead::default(),
            SftpReader::new(self.clone(), self.root.clone(), path.to_owned()),
        ))
    }

    async fn write(&self, path: &str, op: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        if let Some((dir, _)) = path.rsplit_once('/') {
            self.create_dir(dir, OpCreateDir::default()).await?;
        }

        let client = self.connect().await?;

        let mut fs = client.fs();
        fs.set_cwd(&self.root);
        let path = fs.canonicalize(path).await.map_err(parse_sftp_error)?;

        let mut option = client.options();
        option.create(true);
        if op.append() {
            option.append(true);
        } else {
            option.write(true);
        }

        let file = option.open(path).await.map_err(parse_sftp_error)?;

        Ok((RpWrite::new(), SftpWriter::new(file)))
    }

    async fn delete(&self, path: &str, _: OpDelete) -> Result<RpDelete> {
        let client = self.connect().await?;

        let mut fs = client.fs();
        fs.set_cwd(&self.root);

        let res = if path.ends_with('/') {
            fs.remove_dir(path).await
        } else {
            fs.remove_file(path).await
        };

        match res {
            Ok(()) => Ok(RpDelete::default()),
            Err(e) if !is_not_found(&e) => Ok(RpDelete::default()),
            Err(e) => Err(parse_sftp_error(e)),
        }
    }

    async fn list(&self, path: &str, _: OpList) -> Result<(RpList, Self::Lister)> {
        let client = self.connect().await?;
        let mut fs = client.fs();
        fs.set_cwd(&self.root);

        let file_path = format!("./{}", path);

        let dir = match fs.open_dir(&file_path).await {
            Ok(dir) => dir,
            Err(e) => {
                if is_not_found(&e) {
                    return Ok((RpList::default(), None));
                } else {
                    return Err(parse_sftp_error(e));
                }
            }
        }
        .read_dir();

        Ok((
            RpList::default(),
            Some(SftpLister::new(dir, path.to_owned())),
        ))
    }

    async fn copy(&self, from: &str, to: &str, _: OpCopy) -> Result<RpCopy> {
        let client = self.connect().await?;

        let mut fs = client.fs();
        fs.set_cwd(&self.root);

        if let Some((dir, _)) = to.rsplit_once('/') {
            self.create_dir(dir, OpCreateDir::default()).await?;
        }

        let src = fs.canonicalize(from).await.map_err(parse_sftp_error)?;
        let dst = fs.canonicalize(to).await.map_err(parse_sftp_error)?;
        let mut src_file = client.open(&src).await.map_err(parse_sftp_error)?;
        let mut dst_file = client.create(dst).await.map_err(parse_sftp_error)?;

        src_file
            .copy_all_to(&mut dst_file)
            .await
            .map_err(parse_sftp_error)?;

        Ok(RpCopy::default())
    }

    async fn rename(&self, from: &str, to: &str, _: OpRename) -> Result<RpRename> {
        let client = self.connect().await?;

        let mut fs = client.fs();
        fs.set_cwd(&self.root);

        if let Some((dir, _)) = to.rsplit_once('/') {
            self.create_dir(dir, OpCreateDir::default()).await?;
        }
        fs.rename(from, to).await.map_err(parse_sftp_error)?;

        Ok(RpRename::default())
    }
}

impl SftpBackend {
    /// TODO: implement connection pool in the future.
    pub async fn connect(&self) -> Result<Sftp> {
        connect_sftp(
            self.endpoint.as_str(),
            self.root.clone(),
            self.user.clone(),
            self.key.clone(),
            self.known_hosts_strategy.clone(),
        )
        .await
    }
}

async fn connect_sftp(
    endpoint: &str,
    root: String,
    user: Option<String>,
    key: Option<String>,
    known_hosts_strategy: KnownHosts,
) -> Result<Sftp> {
    let mut session = SessionBuilder::default();

    if let Some(user) = user {
        session.user(user);
    }

    if let Some(key) = &key {
        session.keyfile(key);
    }

    session.known_hosts_check(known_hosts_strategy);

    let session = session.connect(&endpoint).await.map_err(parse_ssh_error)?;

    let sftp = Sftp::from_session(session, SftpOptions::default())
        .await
        .map_err(parse_sftp_error)?;

    if !root.is_empty() {
        let mut fs = sftp.fs();

        let paths = Path::new(&root).components();
        let mut current = PathBuf::new();
        for p in paths {
            current.push(p);
            let res = fs.create_dir(p).await;

            if let Err(e) = res {
                // ignore error if dir already exists
                if !is_sftp_protocol_error(&e) {
                    return Err(parse_sftp_error(e));
                }
            }
            fs.set_cwd(&current);
        }
    }

    debug!("sftp connection created at {}", root);

    Ok(sftp)
}
