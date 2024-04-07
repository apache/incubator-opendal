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
use std::sync::Arc;

use async_trait::async_trait;

use log::debug;
use serde::Deserialize;

use super::core::*;


use super::lister::PcloudLister;
use super::writer::PcloudWriter;
use super::writer::PcloudWriters;
use crate::raw::*;
use crate::services::pcloud::reader::PcloudReader;
use crate::*;

/// Config for backblaze Pcloud services support.
#[derive(Default, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct PcloudConfig {
    /// root of this backend.
    ///
    /// All operations will happen under this root.
    pub root: Option<String>,
    ///pCloud  endpoint address.
    pub endpoint: String,
    /// pCloud username.
    pub username: Option<String>,
    /// pCloud password.
    pub password: Option<String>,
}

impl Debug for PcloudConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Config");

        ds.field("root", &self.root);
        ds.field("endpoint", &self.endpoint);
        ds.field("username", &self.username);

        ds.finish()
    }
}

/// [pCloud](https://www.pcloud.com/) services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct PcloudBuilder {
    config: PcloudConfig,

    http_client: Option<HttpClient>,
}

impl Debug for PcloudBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("PcloudBuilder");

        d.field("config", &self.config);
        d.finish_non_exhaustive()
    }
}

impl PcloudBuilder {
    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(&mut self, root: &str) -> &mut Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Pcloud endpoint.
    /// https://api.pcloud.com for United States and https://eapi.pcloud.com for Europe
    /// ref to [doc.pcloud.com](https://docs.pcloud.com/)
    ///
    /// It is required. e.g. `https://api.pcloud.com`
    pub fn endpoint(&mut self, endpoint: &str) -> &mut Self {
        self.config.endpoint = endpoint.to_string();

        self
    }

    /// Pcloud username.
    ///
    /// It is required. your pCloud login email, e.g. `example@gmail.com`
    pub fn username(&mut self, username: &str) -> &mut Self {
        self.config.username = if username.is_empty() {
            None
        } else {
            Some(username.to_string())
        };

        self
    }

    /// Pcloud password.
    ///
    /// It is required. your pCloud login password, e.g. `password`
    pub fn password(&mut self, password: &str) -> &mut Self {
        self.config.password = if password.is_empty() {
            None
        } else {
            Some(password.to_string())
        };

        self
    }

    /// Specify the http client that used by this service.
    ///
    /// # Notes
    ///
    /// This API is part of OpenDAL's Raw API. `HttpClient` could be changed
    /// during minor updates.
    pub fn http_client(&mut self, client: HttpClient) -> &mut Self {
        self.http_client = Some(client);
        self
    }
}

impl Builder for PcloudBuilder {
    const SCHEME: Scheme = Scheme::Pcloud;
    type Accessor = PcloudBackend;

    /// Converts a HashMap into an PcloudBuilder instance.
    ///
    /// # Arguments
    ///
    /// * `map` - A HashMap containing the configuration values.
    ///
    /// # Returns
    ///
    /// Returns an instance of PcloudBuilder.
    fn from_map(map: HashMap<String, String>) -> Self {
        // Deserialize the configuration from the HashMap.
        let config = PcloudConfig::deserialize(ConfigDeserializer::new(map))
            .expect("config deserialize must succeed");

        // Create an PcloudBuilder instance with the deserialized config.
        PcloudBuilder {
            config,
            http_client: None,
        }
    }

    /// Builds the backend and returns the result of PcloudBackend.
    fn build(&mut self) -> Result<Self::Accessor> {
        debug!("backend build started: {:?}", &self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", &root);

        // Handle endpoint.
        if self.config.endpoint.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", Scheme::Pcloud));
        }

        debug!("backend use endpoint {}", &self.config.endpoint);

        let username = match &self.config.username {
            Some(username) => Ok(username.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "username is empty")
                .with_operation("Builder::build")
                .with_context("service", Scheme::Pcloud)),
        }?;

        let password = match &self.config.password {
            Some(password) => Ok(password.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "password is empty")
                .with_operation("Builder::build")
                .with_context("service", Scheme::Pcloud)),
        }?;

        let client = if let Some(client) = self.http_client.take() {
            client
        } else {
            HttpClient::new().map_err(|err| {
                err.with_operation("Builder::build")
                    .with_context("service", Scheme::Pcloud)
            })?
        };

        Ok(PcloudBackend {
            core: Arc::new(PcloudCore {
                root,
                endpoint: self.config.endpoint.clone(),
                username,
                password,
                client,
            }),
        })
    }
}

/// Backend for Pcloud services.
#[derive(Debug, Clone)]
pub struct PcloudBackend {
    core: Arc<PcloudCore>,
}

#[async_trait]
impl Accessor for PcloudBackend {
    type Reader = PcloudReader;
    type Writer = PcloudWriters;
    type Lister = oio::PageLister<PcloudLister>;
    type BlockingReader = ();
    type BlockingWriter = ();
    type BlockingLister = ();

    fn info(&self) -> AccessorInfo {
        let mut am = AccessorInfo::default();
        am.set_scheme(Scheme::Pcloud)
            .set_root(&self.core.root)
            .set_native_capability(Capability {
                stat: true,

                create_dir: true,

                read: true,

                write: true,

                delete: true,
                rename: true,
                copy: true,

                list: true,

                ..Default::default()
            });

        am
    }

    async fn create_dir(&self, path: &str, _: OpCreateDir) -> Result<RpCreateDir> {
        self.core.ensure_dir_exists(path).await?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, path: &str, _args: OpStat) -> Result<RpStat> {
        self.core.stat(path).await.map(RpStat::new)
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let link = self.core.get_file_link(path).await?;

        Ok((
            RpRead::default(),
            PcloudReader::new(self.core.clone(), &link, args),
        ))
    }

    async fn write(&self, path: &str, _args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let writer = PcloudWriter::new(self.core.clone(), path.to_string());

        let w = oio::OneShotWriter::new(writer);

        Ok((RpWrite::default(), w))
    }

    async fn delete(&self, path: &str, _: OpDelete) -> Result<RpDelete> {
        if path.ends_with('/') {
            self.core.delete_folder(path).await?
        } else {
            self.core.delete_file(path).await?
        };

        Ok(RpDelete::default())
    }

    async fn list(&self, path: &str, _args: OpList) -> Result<(RpList, Self::Lister)> {
        let l = PcloudLister::new(self.core.clone(), path);
        Ok((RpList::default(), oio::PageLister::new(l)))
    }

    async fn copy(&self, from: &str, to: &str, _args: OpCopy) -> Result<RpCopy> {
        self.core.ensure_dir_exists(to).await?;

        if from.ends_with('/') {
            self.core.copy_folder(from, to).await?
        } else {
            self.core.copy_file(from, to).await?
        };

        Ok(RpCopy::default())
    }

    async fn rename(&self, from: &str, to: &str, _args: OpRename) -> Result<RpRename> {
        self.core.ensure_dir_exists(to).await?;

        if from.ends_with('/') {
            self.core.rename_folder(from, to).await?
        } else {
            self.core.rename_file(from, to).await?
        };

        Ok(RpRename::default())
    }
}
