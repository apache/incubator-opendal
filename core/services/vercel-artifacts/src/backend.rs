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

use super::core::parse_error;
use super::core::{ErrorContext, VercelArtifactsCore};
use super::reader::*;
use super::writer::VercelArtifactsWriter;
use opendal_core::raw::*;
use opendal_core::*;

#[doc = include_str!("docs.md")]
use std::fmt::Debug;

use super::VERCEL_ARTIFACTS_SCHEME;
use super::config::VercelArtifactsConfig;

/// [Vercel Cache](https://vercel.com/docs/concepts/monorepos/remote-caching) backend support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct VercelArtifactsBuilder {
    pub(super) config: VercelArtifactsConfig,
}

impl Debug for VercelArtifactsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VercelArtifactsBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl VercelArtifactsBuilder {
    /// set the bearer access token for Vercel
    ///
    /// default: no access token, which leads to failure
    pub fn access_token(mut self, access_token: &str) -> Self {
        self.config.access_token = Some(access_token.to_string());
        self
    }

    /// Set the endpoint for the Vercel artifacts API.
    ///
    /// Default: `https://api.vercel.com`
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.trim_end_matches('/').to_string());
        }
        self
    }

    /// Set the Vercel team ID.
    ///
    /// When set, the `teamId` query parameter is appended to all requests.
    pub fn team_id(mut self, team_id: &str) -> Self {
        if !team_id.is_empty() {
            self.config.team_id = Some(team_id.to_string());
        }
        self
    }

    /// Set the Vercel team slug.
    ///
    /// When set, the `slug` query parameter is appended to all requests.
    pub fn team_slug(mut self, team_slug: &str) -> Self {
        if !team_slug.is_empty() {
            self.config.team_slug = Some(team_slug.to_string());
        }
        self
    }
}

impl Builder for VercelArtifactsBuilder {
    type Config = VercelArtifactsConfig;

    fn build(self) -> Result<impl Service> {
        let info = ServiceInfo::new(VERCEL_ARTIFACTS_SCHEME, "", "");
        let capability = Capability {
            stat: true,

            read: true,
            read_with_suffix: true,

            write: true,

            shared: true,

            ..Default::default()
        };

        let access_token = self
            .config
            .access_token
            .ok_or_else(|| Error::new(ErrorKind::ConfigInvalid, "access_token not set"))?;

        let endpoint = self
            .config
            .endpoint
            .unwrap_or_else(|| "https://api.vercel.com".to_string());

        let mut query_params = Vec::new();
        if let Some(team_id) = &self.config.team_id {
            query_params.push(format!("teamId={team_id}"));
        }
        if let Some(slug) = &self.config.team_slug {
            query_params.push(format!("slug={slug}"));
        }
        let query_string = if query_params.is_empty() {
            String::new()
        } else {
            format!("?{}", query_params.join("&"))
        };

        Ok(VercelArtifactsBackend {
            core: Arc::new(VercelArtifactsCore {
                info,
                capability,
                access_token,
                endpoint,
                query_string,
            }),
        })
    }
}

#[derive(Clone, Debug)]
pub struct VercelArtifactsBackend {
    pub core: Arc<VercelArtifactsCore>,
}

impl Service for VercelArtifactsBackend {
    type Reader = oio::StreamReader<VercelArtifactsReader>;
    type Writer = oio::OneShotWriter<VercelArtifactsWriter>;
    type Lister = ();
    type Deleter = ();
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
        _ctx: &OperationContext,
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let response = self.core.vercel_artifacts_stat(ctx, path).await?;

        let status = response.status();

        match status {
            StatusCode::OK => {
                let meta = parse_into_metadata(path, response.headers())?;
                Ok(RpStat::new(meta))
            }

            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("HeadArtifact")),
                response,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<VercelArtifactsReader> = {
            Ok(oio::StreamReader::new(VercelArtifactsReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: oio::OneShotWriter<VercelArtifactsWriter> = {
            Ok(oio::OneShotWriter::new(VercelArtifactsWriter::new(
                self.core.clone(),
                ctx.clone(),
                args,
                path.to_string(),
            )))
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn list(&self, _ctx: &OperationContext, _path: &str, _args: OpList) -> Result<Self::Lister> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
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
