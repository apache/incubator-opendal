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

use super::MEMORY_SCHEME;
use super::config::MemoryConfig;
use super::core::*;
use super::deleter::MemoryDeleter;
use super::lister::MemoryLister;
use super::writer::MemoryWriter;
use crate::raw::oio;
use crate::raw::*;
use crate::*;

/// In memory service support. (BTreeMap Based)
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct MemoryBuilder {
    pub(super) config: MemoryConfig,
}

impl MemoryBuilder {
    /// Set the root for BTreeMap.
    pub fn root(mut self, path: &str) -> Self {
        self.config.root = Some(path.into());
        self
    }
}

impl Builder for MemoryBuilder {
    type Config = MemoryConfig;

    fn build(self) -> Result<impl Service> {
        let root = normalize_root(self.config.root.as_deref().unwrap_or("/"));

        let core = MemoryCore::new();
        Ok(MemoryBackend::new(core).with_normalized_root(root))
    }
}

/// MemoryBackend implements [`Service`] for the in-memory map.
#[derive(Debug, Clone)]
pub struct MemoryBackend {
    core: Arc<MemoryCore>,
    root: String,
    info: ServiceInfo,
    capability: Capability,
}

impl MemoryBackend {
    fn new(core: MemoryCore) -> Self {
        let capability = Capability {
            read: true,
            read_with_suffix: true,
            write: true,
            write_can_empty: true,
            write_with_cache_control: true,
            write_with_content_type: true,
            write_with_content_disposition: true,
            write_with_content_encoding: true,
            write_with_if_not_exists: true,
            delete: true,
            stat: true,
            list: true,
            list_with_recursive: true,
            ..Default::default()
        };

        Self {
            info: ServiceInfo::new(MEMORY_SCHEME, "/", format!("{:p}", Arc::as_ptr(&core.data))),
            core: Arc::new(core),
            root: "/".to_string(),
            capability,
        }
    }

    fn with_normalized_root(mut self, root: String) -> Self {
        self.info = ServiceInfo::new(MEMORY_SCHEME, root.clone(), self.info.name());
        self.root = root;
        self
    }
}

impl Service for MemoryBackend {
    type Reader = oio::StreamReader<MemoryReader>;
    type Writer = MemoryWriter;
    type Lister = oio::HierarchyLister<MemoryLister>;
    type Deleter = oio::OneShotDeleter<MemoryDeleter>;
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
        _: &OperationContext,
        _: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, _: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let p = build_abs_path(&self.root, path);

        if p == build_abs_path(&self.root, "") {
            Ok(RpStat::new(MetadataBuilder::dir().build()))
        } else {
            match self.core.get(&p)? {
                Some(value) => Ok(RpStat::new(value.metadata)),
                None => Err(Error::new(
                    ErrorKind::NotFound,
                    "memory doesn't have this path",
                )),
            }
        }
    }

    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        Ok(oio::StreamReader::new(MemoryReader::new(
            self.clone(),
            path,
            args,
        )))
    }

    fn write(&self, _ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let p = build_abs_path(&self.root, path);
        Ok(MemoryWriter::new(self.core.clone(), p, args))
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        Ok(oio::OneShotDeleter::new(MemoryDeleter::new(
            self.core.clone(),
            self.root.clone(),
        )))
    }

    fn list(&self, _ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let p = build_abs_path(&self.root, path);
        let keys = self.core.scan(&p)?;
        let lister = MemoryLister::new(&self.root, keys);
        let lister = oio::HierarchyLister::new(lister, path, args.recursive());

        Ok(lister)
    }

    fn copy(&self, _: &OperationContext, _: &str, _: &str, _: OpCopy) -> Result<Self::Copier> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn rename(
        &self,
        _: &OperationContext,
        _: &str,
        _: &str,
        _: OpRename,
    ) -> Result<RpRename> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}

/// Reader returned by this backend.
pub struct MemoryReader {
    backend: MemoryBackend,
    path: String,
}

impl MemoryReader {
    fn new(backend: MemoryBackend, path: &str, _: OpRead) -> Self {
        Self {
            backend,
            path: path.to_string(),
        }
    }
}

impl oio::StreamRead for MemoryReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let p = build_abs_path(&backend.root, path);

        let value = match backend.core.get(&p)? {
            Some(value) => value,
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "memory doesn't have this path",
                ));
            }
        };

        let total_size = value.content.len() as u64;
        let content = value
            .content
            .slice(range.to_content_range(value.content.len())?);
        let metadata = MetadataBuilder::file(total_size);
        let metadata = metadata.build();
        Ok((
            RpRead::new(metadata),
            Box::new(content) as Box<dyn oio::ReadStreamDyn>,
        ))
    }
}
