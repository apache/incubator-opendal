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

use foyer::HybridCache;
use log::debug;

use super::FOYER_SCHEME;
use super::FoyerKey;
use super::FoyerValue;
use super::config::FoyerConfig;
use super::core::FoyerCore;
use super::deleter::FoyerDeleter;
use super::reader::*;
use super::writer::FoyerWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// [foyer](https://github.com/foyer-rs/foyer) backend support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct FoyerBuilder {
    pub(super) config: FoyerConfig,
    pub(super) cache: Option<Arc<HybridCache<FoyerKey, FoyerValue>>>,
}

impl Debug for FoyerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoyerBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl FoyerBuilder {
    /// Create a new [`FoyerBuilder`] with default settings.
    ///
    /// The cache will be lazily initialized when first accessed if not provided via [`Self::cache`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use opendal_service_foyer::Foyer;
    ///
    /// let builder = Foyer::new()
    ///     .memory(64 * 1024 * 1024)
    ///     .root("/cache");
    /// ```
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    /// Set the name of this cache instance.
    pub fn name(mut self, name: &str) -> Self {
        if !name.is_empty() {
            self.config.name = Some(name.to_owned());
        }
        self
    }

    /// Set a pre-built [`foyer::HybridCache`] instance.
    ///
    /// If provided, this cache will be used directly. Otherwise, a cache will be
    /// lazily initialized using the configured memory size.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use opendal_service_foyer::Foyer;
    /// use foyer::HybridCacheBuilder;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let cache = HybridCacheBuilder::new()
    ///     .memory(64 * 1024 * 1024)
    ///     .storage()
    ///     .build()
    ///     .await?;
    ///
    /// let builder = Foyer::new().cache(cache);
    /// # Ok(())
    /// # }
    /// ```
    pub fn cache(mut self, cache: HybridCache<FoyerKey, FoyerValue>) -> Self {
        self.cache = Some(Arc::new(cache));
        self
    }

    /// Set the root path of this backend.
    ///
    /// All operations will be relative to this root path.
    pub fn root(mut self, path: &str) -> Self {
        self.config.root = if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        };
        self
    }

    /// Set the memory capacity in bytes for the cache.
    ///
    /// This is used when the cache is lazily initialized (i.e., when no pre-built cache
    /// is provided via [`Self::cache`]).
    ///
    /// Default is 1 GiB (1024 * 1024 * 1024 bytes).
    pub fn memory(mut self, size: usize) -> Self {
        self.config.memory = Some(size);
        self
    }

    /// Set the disk cache directory path.
    ///
    /// Enables hybrid cache with disk storage. When memory cache is full, data will
    /// be persisted to this directory.
    pub fn disk_path(mut self, path: &str) -> Self {
        self.config.disk_path = if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        };
        self
    }

    /// Set the disk cache total capacity in bytes.
    ///
    /// Only used when `disk_path` is set.
    pub fn disk_capacity(mut self, size: usize) -> Self {
        self.config.disk_capacity = Some(size);
        self
    }

    /// Set the individual cache file size in bytes.
    ///
    /// Default is 1 MiB (1024 * 1024 bytes).
    /// Only used when `disk_path` is set.
    pub fn disk_file_size(mut self, size: usize) -> Self {
        self.config.disk_file_size = Some(size);
        self
    }

    /// Set the recovery mode when starting the cache.
    ///
    /// Valid values: "none" (default), "quiet", "strict".
    /// - "none": Don't recover from disk
    /// - "quiet": Recover and skip errors
    /// - "strict": Recover and panic on errors
    pub fn recover_mode(mut self, mode: &str) -> Self {
        if !mode.is_empty() {
            self.config.recover_mode = Some(mode.to_string());
        }
        self
    }

    /// Set the number of shards for concurrent access.
    ///
    /// Default is 1. Higher values improve concurrency but increase overhead.
    pub fn shards(mut self, count: usize) -> Self {
        self.config.shards = Some(count);
        self
    }
}

impl Builder for FoyerBuilder {
    type Config = FoyerConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(
            self.config
                .root
                .clone()
                .unwrap_or_else(|| "/".to_string())
                .as_str(),
        );

        let mut core = FoyerCore::new(self.config.clone());
        if let Some(cache) = self.cache {
            core = core.with_cache(cache.clone());
        }

        debug!("backend build finished: {:?}", self.config);

        Ok(FoyerBackend::new(core).with_normalized_root(root))
    }
}

#[derive(Debug, Clone)]
pub struct FoyerBackend {
    pub(crate) core: Arc<FoyerCore>,
    pub(crate) root: String,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl FoyerBackend {
    fn new(core: FoyerCore) -> Self {
        let info = ServiceInfo::new(FOYER_SCHEME, "/", core.name().unwrap_or("foyer"));
        let capability = Capability {
            read: true,
            write: true,
            write_can_empty: true,
            delete: true,
            stat: true,
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

impl Service for FoyerBackend {
    type Reader = oio::StreamReader<FoyerReader>;
    type Writer = FoyerWriter;
    type Lister = ();
    type Deleter = oio::OneShotDeleter<FoyerDeleter>;
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
            match self.core.get(&p).await? {
                Some(bs) => Ok(RpStat::new({
                    let metadata = MetadataBuilder::file(bs.len() as u64);
                    metadata.build()
                })),
                None => Err(Error::new(ErrorKind::NotFound, "key not found in foyer")),
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<FoyerReader> = {
            Ok(oio::StreamReader::new(FoyerReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: FoyerWriter = {
            let p = build_abs_path(&self.root, path);
            Ok(FoyerWriter::new(self.core.clone(), p))
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<FoyerDeleter> = {
            Ok(oio::OneShotDeleter::new(FoyerDeleter::new(
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
