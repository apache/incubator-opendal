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
use log::debug;
use opendal_core::raw::*;
use opendal_core::*;

use super::LAKEFS_SCHEME;
use super::config::LakefsConfig;
use super::core::LakefsStatus;
use super::core::parse_error;
use super::core::{ErrorContext, LakefsCore};
use super::deleter::LakefsDeleter;
use super::lister::LakefsLister;
use super::reader::*;
use super::writer::LakefsWriter;

/// [Lakefs](https://docs.lakefs.io/reference/api.html#/)'s API support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct LakefsBuilder {
    pub(super) config: LakefsConfig,
}

impl LakefsBuilder {
    /// Set the endpoint of this backend.
    ///
    /// endpoint must be full uri.
    ///
    /// This is required.
    /// - `http://127.0.0.1:8000` (lakefs daemon in local)
    /// - `https://my-lakefs.example.com` (lakefs server)
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.to_string());
        }
        self
    }

    /// Set username of this backend. This is required.
    pub fn username(mut self, username: &str) -> Self {
        if !username.is_empty() {
            self.config.username = Some(username.to_string());
        }
        self
    }

    /// Set password of this backend. This is required.
    pub fn password(mut self, password: &str) -> Self {
        if !password.is_empty() {
            self.config.password = Some(password.to_string());
        }
        self
    }

    /// Set branch of this backend or a commit ID. Default is main.
    ///
    /// Branch can be a branch name.
    ///
    /// For example, branch can be:
    /// - main
    /// - 1d0c4eb
    pub fn branch(mut self, branch: &str) -> Self {
        if !branch.is_empty() {
            self.config.branch = Some(branch.to_string());
        }
        self
    }

    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(mut self, root: &str) -> Self {
        if !root.is_empty() {
            self.config.root = Some(root.to_string());
        }
        self
    }

    /// Set the repository of this backend.
    ///
    /// This is required.
    pub fn repository(mut self, repository: &str) -> Self {
        if !repository.is_empty() {
            self.config.repository = Some(repository.to_string());
        }
        self
    }
}

impl Builder for LakefsBuilder {
    type Config = LakefsConfig;

    /// Build a LakefsBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let endpoint = match self.config.endpoint {
            Some(endpoint) => Ok(endpoint.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", LAKEFS_SCHEME)),
        }?;
        debug!("backend use endpoint: {:?}", endpoint);

        let repository = match &self.config.repository {
            Some(repository) => Ok(repository.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "repository is empty")
                .with_operation("Builder::build")
                .with_context("service", LAKEFS_SCHEME)),
        }?;
        debug!("backend use repository: {}", repository);

        let branch = match &self.config.branch {
            Some(branch) => branch.clone(),
            None => "main".to_string(),
        };
        debug!("backend use branch: {}", branch);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root: {}", root);

        let username = match &self.config.username {
            Some(username) => Ok(username.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "username is empty")
                .with_operation("Builder::build")
                .with_context("service", LAKEFS_SCHEME)),
        }?;

        let password = match &self.config.password {
            Some(password) => Ok(password.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "password is empty")
                .with_operation("Builder::build")
                .with_context("service", LAKEFS_SCHEME)),
        }?;

        Ok(LakefsBackend {
            core: Arc::new(LakefsCore {
                info: ServiceInfo::new(LAKEFS_SCHEME, "", ""),
                capability: Capability {
                    stat: true,

                    list: true,

                    read: true,
                    read_with_suffix: true,
                    write: true,
                    delete: true,
                    copy: true,
                    shared: true,
                    ..Default::default()
                },
                endpoint,
                repository,
                branch,
                root,
                username,
                password,
            }),
        })
    }
}

/// Backend for Lakefs service
#[derive(Debug, Clone)]
pub struct LakefsBackend {
    pub(crate) core: Arc<LakefsCore>,
}

impl Service for LakefsBackend {
    type Reader = oio::StreamReader<LakefsReader>;
    type Writer = oio::OneShotWriter<LakefsWriter>;
    type Lister = oio::PageLister<LakefsLister>;
    type Deleter = oio::OneShotDeleter<LakefsDeleter>;
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
        _ctx: &OperationContext,
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        // Stat root always returns a DIR.
        if path == "/" {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        let resp = self.core.get_object_metadata(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let decoded_response: LakefsStatus =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                // Use the helper function to parse LakefsStatus into Metadata
                let meta = LakefsCore::parse_lakefs_status_into_metadata(&decoded_response)?;

                Ok(RpStat::new(meta))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("StatObject")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<LakefsReader> = {
            Ok(oio::StreamReader::new(LakefsReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<LakefsLister> = {
            let l = LakefsLister::new(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
                args.limit(),
                args.start_after(),
                args.recursive(),
            );

            Ok(oio::PageLister::new(l))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: oio::OneShotWriter<LakefsWriter> = {
            Ok(oio::OneShotWriter::new(LakefsWriter::new(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
                args,
            )))
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<LakefsDeleter> = {
            Ok(oio::OneShotDeleter::new(LakefsDeleter::new(
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

            let resp = core.copy_object(&ctx, &from, &to).await?;
            let status = resp.status();

            match status {
                StatusCode::CREATED => Ok(MetadataBuilder::file(source_size).build()),
                _ => Err(parse_error(
                    ErrorContext::new(ServiceOperation("CopyObject")),
                    resp,
                )),
            }
        }))
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
