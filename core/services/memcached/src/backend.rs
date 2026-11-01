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

use std::borrow::Cow;
use std::sync::Arc;
use url::Url;

use opendal_core::raw::*;
use opendal_core::*;

use super::MEMCACHED_SCHEME;
use super::config::MemcachedConfig;
use super::core::*;
use super::deleter::MemcachedDeleter;
use super::reader::*;
use super::writer::MemcachedWriter;

/// [Memcached](https://memcached.org/) service support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct MemcachedBuilder {
    pub(super) config: MemcachedConfig,
    pub(super) default_ttl: Option<Duration>,
}

impl MemcachedBuilder {
    /// set the network address of memcached service.
    ///
    /// For example: "tcp://localhost:11211"
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            self.config.endpoint = Some(endpoint.to_owned());
        }
        self
    }

    /// set the working directory, all operations will be performed under it.
    ///
    /// default: "/"
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// set the username.
    pub fn username(mut self, username: &str) -> Self {
        self.config.username = Some(username.to_string());
        self
    }

    /// set the password.
    pub fn password(mut self, password: &str) -> Self {
        self.config.password = Some(password.to_string());
        self
    }

    /// Set the default ttl for memcached services.
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Sets the maximum number of connections managed by the pool.
    ///
    /// Defaults to 10.
    ///
    /// # Panics
    ///
    /// Will panic if `max_size` is 0.
    #[must_use]
    pub fn connection_pool_max_size(mut self, max_size: usize) -> Self {
        assert!(max_size > 0, "max_size must be greater than zero!");
        self.config.connection_pool_max_size = Some(max_size);
        self
    }
}

impl Builder for MemcachedBuilder {
    type Config = MemcachedConfig;

    fn build(self) -> Result<impl Service> {
        let default_ttl = match self.default_ttl {
            Some(ttl) => Some(ttl),
            None => self
                .config
                .default_ttl
                .map(signed_duration_to_duration)
                .transpose()?,
        };
        if let Some(ttl) = default_ttl
            && ttl.as_secs() > MAX_RELATIVE_EXPIRATION_SECS
        {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "default_ttl is larger than 30 days, which memcached reads as an absolute \
                 unix timestamp rather than an offset, storing the entry already expired",
            )
            .with_context("service", MEMCACHED_SCHEME)
            .with_context("default_ttl", ttl.as_secs().to_string()));
        }

        let endpoint_raw = self.config.endpoint.clone().ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_context("service", MEMCACHED_SCHEME)
        })?;

        let url_str = if !endpoint_raw.contains("://") {
            Cow::Owned(format!("tcp://{}", endpoint_raw))
        } else {
            Cow::Borrowed(endpoint_raw.as_str())
        };

        let parsed = Url::parse(&url_str).map_err(|err| {
            Error::new(ErrorKind::ConfigInvalid, "endpoint is invalid")
                .with_context("service", MEMCACHED_SCHEME)
                .with_context("endpoint", &endpoint_raw)
                .set_source(err)
        })?;

        let endpoint = match parsed.scheme() {
            "tcp" => {
                let host = parsed.host_str().ok_or_else(|| {
                    Error::new(ErrorKind::ConfigInvalid, "tcp endpoint doesn't have host")
                        .with_context("service", MEMCACHED_SCHEME)
                        .with_context("endpoint", &endpoint_raw)
                })?;
                let port = parsed.port().ok_or_else(|| {
                    Error::new(ErrorKind::ConfigInvalid, "tcp endpoint doesn't have port")
                        .with_context("service", MEMCACHED_SCHEME)
                        .with_context("endpoint", &endpoint_raw)
                })?;
                Endpoint::Tcp(format!("{host}:{port}"))
            }

            #[cfg(unix)]
            "unix" => {
                let path = parsed.path();
                if path.is_empty() {
                    return Err(Error::new(
                        ErrorKind::ConfigInvalid,
                        "unix endpoint doesn't have path",
                    )
                    .with_context("service", MEMCACHED_SCHEME)
                    .with_context("endpoint", &endpoint_raw));
                }
                Endpoint::Unix(path.to_string())
            }

            #[cfg(not(unix))]
            "unix" => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "unix socket is not supported on this platform",
                )
                .with_context("service", MEMCACHED_SCHEME)
                .with_context("endpoint", &endpoint_raw));
            }

            scheme => {
                return Err(Error::new(
                    ErrorKind::ConfigInvalid,
                    "endpoint is using invalid scheme, only tcp and unix are supported",
                )
                .with_context("service", MEMCACHED_SCHEME)
                .with_context("endpoint", &endpoint_raw)
                .with_context("scheme", scheme));
            }
        };

        let root = normalize_root(self.config.root.unwrap_or_else(|| "/".to_string()).as_str());

        Ok(MemcachedBackend::new(MemcachedCore::new(
            endpoint,
            self.config.username,
            self.config.password,
            default_ttl,
            self.config.connection_pool_max_size,
        ))
        .with_normalized_root(root))
    }
}

/// Backend for memcached services.
#[derive(Clone, Debug)]
pub struct MemcachedBackend {
    pub(crate) core: Arc<MemcachedCore>,
    pub(crate) root: String,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl MemcachedBackend {
    pub fn new(core: MemcachedCore) -> Self {
        let info = ServiceInfo::new(MEMCACHED_SCHEME, "/", "memcached");
        let capability = Capability {
            read: true,
            stat: true,
            write: true,
            write_can_empty: true,
            delete: true,
            shared: true,
            ..Default::default()
        };

        Self {
            core: Arc::new(core),
            root: "/".to_string(),
            info,
            capability,
        }
    }

    fn with_normalized_root(mut self, root: String) -> Self {
        self.info = self.info.with_root(&root);
        self.root = root;
        self
    }
}

impl Service for MemcachedBackend {
    type Reader = oio::StreamReader<MemcachedReader>;
    type Writer = MemcachedWriter;
    type Lister = ();
    type Deleter = oio::OneShotDeleter<MemcachedDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.info.clone()
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
            "operation is not supported",
        ))
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let p = build_abs_path(&self.root, path);

        if p == build_abs_path(&self.root, "") {
            Ok(RpStat::new(MetadataBuilder::dir().build()))
        } else {
            let bs = self.core.get(&p).await?;
            match bs {
                Some(bs) => Ok(RpStat::new({
                    let metadata = MetadataBuilder::file(bs.len() as u64);
                    metadata.build()
                })),
                None => Err(Error::new(ErrorKind::NotFound, "kv not found in memcached")),
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<MemcachedReader> = {
            Ok(oio::StreamReader::new(MemcachedReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: MemcachedWriter = {
            let p = build_abs_path(&self.root, path);
            Ok(MemcachedWriter::new(self.core.clone(), p))
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<MemcachedDeleter> = {
            Ok(oio::OneShotDeleter::new(MemcachedDeleter::new(
                self.core.clone(),
                self.root.clone(),
            )))
        }?;

        Ok(output)
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// memcached reads an expiration over 30 days as an absolute unix timestamp,
    /// so a longer offset would store the entry already expired. Refuse it at
    /// build time rather than writing data that silently disappears.
    #[test]
    fn build_rejects_a_ttl_over_thirty_days() {
        let thirty_days = Duration::from_secs(MAX_RELATIVE_EXPIRATION_SECS);

        let ok = MemcachedBuilder::default()
            .endpoint("tcp://127.0.0.1:11211")
            .default_ttl(thirty_days)
            .build();
        assert!(ok.is_ok(), "thirty days exactly must still be accepted");

        let err = MemcachedBuilder::default()
            .endpoint("tcp://127.0.0.1:11211")
            .default_ttl(thirty_days + Duration::from_secs(1))
            .build()
            .expect_err("a ttl over thirty days must be rejected");
        assert_eq!(err.kind(), ErrorKind::ConfigInvalid);
    }
}
