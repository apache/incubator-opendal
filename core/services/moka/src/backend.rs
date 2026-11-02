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

use super::MOKA_SCHEME;
use super::config::MokaConfig;
use super::core::*;
use super::deleter::MokaDeleter;
use super::lister::MokaLister;
use super::reader::*;
use super::writer::MokaWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// Type alias of [`moka::future::Cache`](https://docs.rs/moka/latest/moka/future/struct.Cache.html)
pub type MokaCache<K, V> = moka::future::Cache<K, V>;
/// Type alias of [`moka::future::CacheBuilder`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html)
pub type MokaCacheBuilder<K, V> = moka::future::CacheBuilder<K, V, MokaCache<K, V>>;

/// [moka](https://github.com/moka-rs/moka) backend support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct MokaBuilder {
    pub(super) config: MokaConfig,
    pub(super) builder: MokaCacheBuilder<String, MokaValue>,
}

impl Debug for MokaBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MokaBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl MokaBuilder {
    /// Create a [`MokaBuilder`] with the given [`moka::future::CacheBuilder`].
    ///
    /// Refer to [`moka::future::CacheBuilder`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use std::time::Duration;
    /// # use log::debug;
    /// # use moka::notification::RemovalCause;
    /// # use opendal_service_moka::Moka;
    /// # use opendal_service_moka::MokaCacheBuilder;
    /// # use opendal_service_moka::MokaValue;
    /// # use opendal_core::Configurator;
    /// let moka = Moka::new(
    ///     MokaCacheBuilder::<String, MokaValue>::default()
    ///         .name("demo")
    ///         .max_capacity(1000)
    ///         .time_to_live(Duration::from_secs(300))
    ///         .weigher(|k, v| (k.len() + v.content.len()) as u32)
    ///         .eviction_listener(|k: Arc<String>, v: MokaValue, cause: RemovalCause| {
    ///             debug!(
    ///                 "moka cache eviction listener, key = {}, value = {:?}, cause = {:?}",
    ///                 k.as_str(), v.content.to_vec(), cause
    ///             );
    ///         })
    /// );
    /// ```
    pub fn new(builder: MokaCacheBuilder<String, MokaValue>) -> Self {
        Self {
            builder,
            ..Default::default()
        }
    }

    /// Sets the name of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::name`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.name)
    pub fn name(mut self, v: &str) -> Self {
        if !v.is_empty() {
            self.config.name = Some(v.to_owned());
        }
        self
    }

    /// Sets the max capacity of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::max_capacity`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.max_capacity)
    pub fn max_capacity(mut self, v: u64) -> Self {
        if v != 0 {
            self.config.max_capacity = Some(v);
        }
        self
    }

    /// Sets the time to live of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::time_to_live`](https://docs.rs/moka/latest/moka/future/struct.CacheBuilder.html#method.time_to_live)
    pub fn time_to_live(mut self, v: Duration) -> Self {
        if !v.is_zero() {
            self.config.time_to_live = Some(format!("{}s", v.as_secs()));
        }
        self
    }

    /// Sets the time to idle of the cache.
    ///
    /// Refer to [`moka::future::CacheBuilder::time_to_idle`](https://docs.rs/moka/latest/moka/sync/struct.CacheBuilder.html#method.time_to_idle)
    pub fn time_to_idle(mut self, v: Duration) -> Self {
        if !v.is_zero() {
            self.config.time_to_idle = Some(format!("{}s", v.as_secs()));
        }
        self
    }

    /// Set the root path of this backend
    pub fn root(mut self, path: &str) -> Self {
        self.config.root = if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        };
        self
    }
}

impl Builder for MokaBuilder {
    type Config = MokaConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(
            self.config
                .root
                .clone()
                .unwrap_or_else(|| "/".to_string())
                .as_str(),
        );

        let mut builder = self.builder;

        if let Some(v) = &self.config.name {
            builder = builder.name(v);
        }
        if let Some(v) = self.config.max_capacity {
            builder = builder.max_capacity(v);
        }
        if let Some(value) = self.config.time_to_live.as_deref() {
            let duration = signed_to_duration(value)?;
            builder = builder.time_to_live(duration);
        }
        if let Some(value) = self.config.time_to_idle.as_deref() {
            let duration = signed_to_duration(value)?;
            builder = builder.time_to_idle(duration);
        }

        debug!("backend build finished: {:?}", self.config);

        let core = MokaCore {
            cache: builder.build(),
        };

        Ok(MokaBackend::new(core).with_normalized_root(root))
    }
}

#[derive(Debug, Clone)]
pub struct MokaBackend {
    pub(crate) core: Arc<MokaCore>,
    pub(crate) root: String,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl MokaBackend {
    fn new(core: MokaCore) -> Self {
        let info = ServiceInfo::new(MOKA_SCHEME, "/", core.cache.name().unwrap_or("moka"));
        let capability = Capability {
            read: true,
            write: true,
            write_can_empty: true,
            write_with_cache_control: true,
            write_with_content_type: true,
            write_with_content_disposition: true,
            write_with_content_encoding: true,
            delete: true,
            stat: true,
            list: true,
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

impl Service for MokaBackend {
    type Reader = oio::StreamReader<MokaReader>;
    type Writer = MokaWriter;
    type Lister = oio::HierarchyLister<MokaLister>;
    type Deleter = oio::OneShotDeleter<MokaDeleter>;
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
            // Check the exact path first
            match self.core.get(&p).await? {
                Some(value) => {
                    // Use the stored metadata but override mode if necessary
                    let mut metadata = value.metadata.clone().into_builder();
                    // If path ends with '/' but we found a file, return DIR
                    // This is because CompleteLayer's create_dir creates empty files with '/' suffix
                    if p.ends_with('/') && value.metadata.mode() != EntryMode::DIR {
                        metadata.set_dir();
                    }
                    Ok(RpStat::new(metadata.build()))
                }
                None => {
                    // If path ends with '/', check if there are any children
                    if p.ends_with('/') {
                        let has_children = self
                            .core
                            .cache
                            .iter()
                            .any(|kv| kv.0.starts_with(&p) && kv.0.len() > p.len());

                        if has_children {
                            Ok(RpStat::new(MetadataBuilder::dir().build()))
                        } else {
                            Err(Error::new(ErrorKind::NotFound, "key not found in moka"))
                        }
                    } else {
                        Err(Error::new(ErrorKind::NotFound, "key not found in moka"))
                    }
                }
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<MokaReader> = {
            Ok(oio::StreamReader::new(MokaReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: MokaWriter = {
            let p = build_abs_path(&self.root, path);
            Ok(MokaWriter::new(self.core.clone(), p, args))
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<MokaDeleter> = {
            Ok(oio::OneShotDeleter::new(MokaDeleter::new(
                self.core.clone(),
                self.root.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let output: oio::HierarchyLister<MokaLister> = {
            // For moka, we don't distinguish between files and directories
            // Just return the lister to iterate through all matching keys
            let lister = MokaLister::new(self.core.clone(), self.root.clone(), path.to_string());
            let lister = oio::HierarchyLister::new(lister, path, args.recursive());
            Ok(lister)
        }?;

        Ok(output)
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
