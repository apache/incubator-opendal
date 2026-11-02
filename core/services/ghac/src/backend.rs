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

use std::env;
use std::fmt::Debug;
use std::sync::Arc;

use http::StatusCode;
use log::debug;
use sha2::Digest;

use super::GHAC_SCHEME;
use super::config::GhacConfig;
use super::core::GhacCore;
use super::core::parse_error;
use super::core::*;
use super::reader::*;
use super::writer::GhacLazyWriter;
use opendal_core::raw::*;
use opendal_core::*;

fn value_or_env(
    explicit_value: Option<String>,
    env_var_name: &str,
    operation: &'static str,
) -> Result<String> {
    if let Some(value) = explicit_value {
        return Ok(value);
    }

    env::var(env_var_name).map_err(|err| {
        let text = format!("{env_var_name} not found, maybe not in github action environment?");
        Error::new(ErrorKind::ConfigInvalid, text)
            .with_operation(operation)
            .set_source(err)
    })
}

/// GitHub Action Cache Services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct GhacBuilder {
    pub(super) config: GhacConfig,
}

impl Debug for GhacBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GhacBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl GhacBuilder {
    /// set the working directory root of backend
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// set the version that used by cache.
    ///
    /// The version is the unique value that provides namespacing.
    /// It's better to make sure this value is only used by this backend.
    ///
    /// If not set, we will use `opendal` as default.
    pub fn version(mut self, version: &str) -> Self {
        if !version.is_empty() {
            self.config.version = Some(version.to_string())
        }

        self
    }

    /// Set the endpoint for ghac service.
    ///
    /// For example, this is provided as the `ACTIONS_CACHE_URL` environment variable by the GHA runner.
    ///
    /// Default: the value of the `ACTIONS_CACHE_URL` environment variable.
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.to_string())
        }
        self
    }

    /// Set the runtime token for ghac service.
    ///
    /// For example, this is provided as the `ACTIONS_RUNTIME_TOKEN` environment variable by the GHA
    /// runner.
    ///
    /// Default: the value of the `ACTIONS_RUNTIME_TOKEN` environment variable.
    pub fn runtime_token(mut self, runtime_token: &str) -> Self {
        if !runtime_token.is_empty() {
            self.config.runtime_token = Some(runtime_token.to_string())
        }
        self
    }
}

impl Builder for GhacBuilder {
    type Config = GhacConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {self:?}");

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let service_version = get_cache_service_version();
        debug!("backend use service version {service_version:?}");

        let mut version = self
            .config
            .version
            .clone()
            .unwrap_or_else(|| "opendal".to_string());
        debug!("backend use version {version}");
        // ghac requires to use hex digest of Sha256 as version.
        if matches!(service_version, GhacVersion::V2) {
            let hash = sha2::Sha256::digest(&version);
            version = format_digest_hex(hash);
        }

        let cache_url = self
            .config
            .endpoint
            .unwrap_or_else(|| get_cache_service_url(service_version));
        if cache_url.is_empty() {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "cache url for ghac not found, maybe not in github action environment?".to_string(),
            ));
        }

        let core = GhacCore {
            info: ServiceInfo::new(GHAC_SCHEME, &root, &version),
            capability: Capability {
                stat: true,

                read: true,

                write: true,
                write_can_multi: true,

                shared: true,

                ..Default::default()
            },
            root,

            cache_url,
            catch_token: value_or_env(
                self.config.runtime_token,
                ACTIONS_RUNTIME_TOKEN,
                "Builder::build",
            )?,
            version,

            service_version,
        };

        Ok(GhacBackend {
            core: Arc::new(core),
        })
    }
}

fn format_digest_hex(digest: impl AsRef<[u8]>) -> String {
    use std::fmt::Write;

    let digest = digest.as_ref();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String must succeed");
    }
    output
}

/// Backend for github action cache services.
#[derive(Debug, Clone)]
pub struct GhacBackend {
    pub(crate) core: Arc<GhacCore>,
}

impl Service for GhacBackend {
    type Reader = oio::StreamReader<GhacReader>;
    type Writer = GhacLazyWriter;
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

    /// Some self-hosted GHES instances are backed by AWS S3 services which only returns
    /// signed url with `GET` method. So we will use `GET` with empty range to simulate
    /// `HEAD` instead.
    ///
    /// In this way, we can support both self-hosted GHES and `github.com`.
    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let resp = self.core.ghac_stat(ctx, path).await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT | StatusCode::RANGE_NOT_SATISFIABLE => {
                let meta = parse_into_metadata(path, resp.headers())?;
                Ok(RpStat::new(meta))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetCacheEntry")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<GhacReader> = {
            Ok(oio::StreamReader::new(GhacReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        Ok(GhacLazyWriter::new(
            self.core.clone(),
            ctx.clone(),
            ctx.executor().clone(),
            path.to_string(),
        ))
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
