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

use log::debug;

use super::GIST_SCHEME;
use super::config::GistConfig;
use super::core::GistCore;
use super::core::parse_metadata;
use super::deleter::GistDeleter;
use super::lister::GistLister;
use super::reader::GistReader;
use super::writer::GistWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// [GitHub Gist](https://docs.github.com/en/rest/gists/gists) services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct GistBuilder {
    pub(super) config: GistConfig,
}

impl Debug for GistBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GistBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl GistBuilder {
    /// Set the GitHub access_token.
    ///
    /// `token` is optional. Without it, only public gists can be read and the
    /// rate limit drops to 60 requests per hour. Updating files requires a token
    /// with the `gist` scope.
    pub fn token(mut self, token: &str) -> Self {
        if !token.is_empty() {
            self.config.token = Some(token.to_string());
        }
        self
    }

    /// Set the identifier of the gist to operate on.
    ///
    /// The gist identifier is the last path segment of a gist URL, for example
    /// `https://gist.github.com/<owner>/<gist_id>`.
    pub fn gist_id(mut self, gist_id: &str) -> Self {
        self.config.gist_id = gist_id.to_string();

        self
    }
}

impl Builder for GistBuilder {
    type Config = GistConfig;

    /// Builds the backend and returns the result of GistBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        if self.config.gist_id.is_empty() {
            return Err(Error::new(ErrorKind::ConfigInvalid, "gist_id is empty")
                .with_operation("Builder::build")
                .with_context("service", GIST_SCHEME));
        }

        Ok(GistBackend {
            core: Arc::new(GistCore {
                info: ServiceInfo::new(GIST_SCHEME, "/", ""),
                token: self.config.token.clone(),
                gist_id: self.config.gist_id.clone(),
            }),
            capability: Capability {
                stat: true,

                read: true,

                write: true,

                delete: true,

                list: true,

                shared: true,

                ..Default::default()
            },
        })
    }
}

/// Backend for GitHub Gist services.
#[derive(Debug, Clone)]
pub struct GistBackend {
    pub(crate) core: Arc<GistCore>,
    capability: Capability,
}

/// Resolve the gist file name a path refers to.
///
/// A gist is a flat set of files: the key is the file name itself, so a path
/// with a directory component has no file to resolve to.
pub(super) fn gist_file_name(path: &str) -> Result<&str> {
    let name = path.trim_matches('/');

    if name.is_empty() {
        return Err(Error::new(
            ErrorKind::Unsupported,
            "the gist root is not a file",
        ));
    }

    if name.contains('/') {
        return Err(Error::new(
            ErrorKind::Unsupported,
            "a gist is a flat set of files, so a path cannot contain '/'",
        ));
    }

    Ok(name)
}

impl Service for GistBackend {
    type Reader = oio::StreamReader<GistReader>;
    type Writer = oio::OneShotWriter<GistWriter>;
    type Lister = oio::PageLister<GistLister>;
    type Deleter = oio::OneShotDeleter<GistDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info.clone()
    }

    fn capability(&self) -> Capability {
        self.capability
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "a gist is a flat set of files and has no directories",
        ))
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        if path.trim_matches('/').is_empty() {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        let name = gist_file_name(path)?;

        let Some(file) = self.core.get_file(ctx, name, Operation::Stat).await? else {
            return Err(Error::new(ErrorKind::NotFound, "gist file does not exist")
                .with_context("path", name));
        };

        Ok(RpStat::new(parse_metadata(&file)))
    }

    fn read(&self, ctx: &OperationContext, path: &str, _args: OpRead) -> Result<Self::Reader> {
        Ok(oio::StreamReader::new(GistReader::new(
            self.core.clone(),
            ctx.clone(),
            path.to_string(),
        )))
    }

    fn write(&self, ctx: &OperationContext, path: &str, _args: OpWrite) -> Result<Self::Writer> {
        Ok(oio::OneShotWriter::new(GistWriter::new(
            self.core.clone(),
            ctx.clone(),
            path.to_string(),
        )))
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        Ok(oio::OneShotDeleter::new(GistDeleter::new(
            self.core.clone(),
            ctx.clone(),
        )))
    }

    fn list(&self, ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        Ok(oio::PageLister::new(GistLister::new(
            self.core.clone(),
            ctx.clone(),
            path.to_string(),
        )))
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
