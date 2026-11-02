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

use log::debug;
use opendal_core::raw::*;
use opendal_core::*;

use super::MINI_MOKA_SCHEME;
use super::config::MiniMokaConfig;
use super::core::*;
use super::deleter::MiniMokaDeleter;
use super::lister::MiniMokaLister;
use super::reader::*;
use super::writer::MiniMokaWriter;

/// [mini-moka](https://github.com/moka-rs/mini-moka) backend support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct MiniMokaBuilder {
    pub(super) config: MiniMokaConfig,
}

impl MiniMokaBuilder {
    /// Create a [`MiniMokaBuilder`] with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the max capacity of the cache.
    ///
    /// Refer to [`mini-moka::sync::CacheBuilder::max_capacity`](https://docs.rs/mini-moka/latest/mini_moka/sync/struct.CacheBuilder.html#method.max_capacity)
    pub fn max_capacity(mut self, v: u64) -> Self {
        if v != 0 {
            self.config.max_capacity = Some(v);
        }
        self
    }

    /// Sets the time to live of the cache.
    ///
    /// Refer to [`mini-moka::sync::CacheBuilder::time_to_live`](https://docs.rs/mini-moka/latest/mini_moka/sync/struct.CacheBuilder.html#method.time_to_live)
    pub fn time_to_live(mut self, v: Duration) -> Self {
        if !v.is_zero() {
            self.config.time_to_live = Some(format!("{}s", v.as_secs()));
        }
        self
    }

    /// Sets the time to idle of the cache.
    ///
    /// Refer to [`mini-moka::sync::CacheBuilder::time_to_idle`](https://docs.rs/mini-moka/latest/mini_moka/sync/struct.CacheBuilder.html#method.time_to_idle)
    pub fn time_to_idle(mut self, v: Duration) -> Self {
        if !v.is_zero() {
            self.config.time_to_idle = Some(format!("{}s", v.as_secs()));
        }
        self
    }

    /// Set root path of this backend
    pub fn root(mut self, path: &str) -> Self {
        self.config.root = if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        };

        self
    }
}

impl Builder for MiniMokaBuilder {
    type Config = MiniMokaConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let mut builder: mini_moka::sync::CacheBuilder<String, MiniMokaValue, _> =
            mini_moka::sync::Cache::builder();

        // Use entries' bytes as capacity weigher.
        builder = builder.weigher(|k, v| (k.len() + v.content.len()) as u32);

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

        let cache = builder.build();

        let root = normalize_root(self.config.root.as_deref().unwrap_or("/"));

        let core = Arc::new(MiniMokaCore { cache });

        debug!("backend build finished: {root}");
        Ok(MiniMokaBackend::new(core, root))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MiniMokaBackend {
    pub(crate) core: Arc<MiniMokaCore>,
    pub(crate) root: String,
    capability: Capability,
}

impl MiniMokaBackend {
    fn new(core: Arc<MiniMokaCore>, root: String) -> Self {
        let capability = Capability {
            stat: true,
            read: true,
            write: true,
            write_can_empty: true,
            delete: true,
            list: true,

            ..Default::default()
        };

        Self {
            core,
            root,
            capability,
        }
    }
}

impl Service for MiniMokaBackend {
    type Reader = oio::StreamReader<MiniMokaReader>;
    type Writer = MiniMokaWriter;
    type Lister = oio::HierarchyLister<MiniMokaLister>;
    type Deleter = oio::OneShotDeleter<MiniMokaDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        ServiceInfo::new(MINI_MOKA_SCHEME, &self.root, "")
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

        // Check if path exists directly in cache
        match self.core.get(&p) {
            Some(value) => {
                let mut metadata = value.metadata.clone().into_builder();
                if p.ends_with('/') {
                    metadata.set_dir();
                } else {
                    metadata.set_file(value.metadata.content_length());
                }
                Ok(RpStat::new(metadata.build()))
            }
            None => {
                if p.ends_with('/') {
                    let is_prefix = self
                        .core
                        .cache
                        .iter()
                        .any(|entry| entry.key().starts_with(&p) && entry.key() != &p);

                    if is_prefix {
                        let mut metadata = MetadataBuilder::unknown();
                        metadata.set_dir();
                        return Ok(RpStat::new(metadata.build()));
                    }
                }

                Err(Error::new(ErrorKind::NotFound, "path not found"))
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, op: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<MiniMokaReader> = {
            Ok(oio::StreamReader::new(MiniMokaReader::new(
                self.clone(),
                path,
                op,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, op: OpWrite) -> Result<Self::Writer> {
        let output: MiniMokaWriter = {
            let p = build_abs_path(&self.root, path);
            let writer = MiniMokaWriter::new(self.core.clone(), p, op);
            Ok(writer)
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<MiniMokaDeleter> = {
            let deleter = oio::OneShotDeleter::new(MiniMokaDeleter::new(
                self.core.clone(),
                self.root.clone(),
            ));
            Ok(deleter)
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, op: OpList) -> Result<Self::Lister> {
        let output: oio::HierarchyLister<MiniMokaLister> = {
            let p = build_abs_path(&self.root, path);

            let mini_moka_lister = MiniMokaLister::new(self.core.clone(), self.root.clone(), p);
            let lister = oio::HierarchyLister::new(mini_moka_lister, path, op.recursive());

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
