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

use super::CACACHE_SCHEME;
use super::config::CacacheConfig;
use super::core::CacacheCore;
use super::deleter::CacacheDeleter;
use super::reader::*;
use super::writer::CacacheWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// cacache service support.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct CacacheBuilder {
    pub(super) config: CacacheConfig,
}

impl CacacheBuilder {
    /// Set the path to the cacache data directory. Will create if not exists.
    pub fn datadir(mut self, path: &str) -> Self {
        self.config.datadir = Some(path.into());
        self
    }
}

impl Builder for CacacheBuilder {
    type Config = CacacheConfig;

    fn build(self) -> Result<impl Service> {
        let datadir_path = self.config.datadir.ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "datadir is required but not set")
                .with_context("service", CACACHE_SCHEME)
        })?;

        let core = CacacheCore {
            path: datadir_path.clone(),
        };

        let info = ServiceInfo::new(CACACHE_SCHEME, "/", &datadir_path);
        let capability = Capability {
            read: true,
            write: true,
            delete: true,
            stat: true,
            rename: false,
            list: false,
            ..Default::default()
        };

        Ok(CacacheBackend {
            core: Arc::new(core),
            info,
            capability,
        })
    }
}

/// Backend for cacache services.
#[derive(Debug, Clone)]
pub struct CacacheBackend {
    pub(crate) core: Arc<CacacheCore>,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl Service for CacacheBackend {
    type Reader = oio::StreamReader<CacacheReader>;
    type Writer = CacacheWriter;
    type Lister = ();
    type Deleter = oio::OneShotDeleter<CacacheDeleter>;
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
        let metadata = self.core.metadata(path).await?;

        match metadata {
            Some(meta) => {
                let mut md = MetadataBuilder::file(meta.size as u64);
                // Convert u128 milliseconds to Timestamp
                let millis = meta.time as i64;
                if let Ok(dt) = Timestamp::from_millisecond(millis) {
                    md.last_modified(dt);
                }
                Ok(RpStat::new(md.build()))
            }
            None => Err(Error::new(ErrorKind::NotFound, "entry not found")),
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<CacacheReader> = {
            Ok(oio::StreamReader::new(CacacheReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: CacacheWriter =
            { Ok(CacacheWriter::new(self.core.clone(), path.to_string())) }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<CacacheDeleter> = {
            Ok(oio::OneShotDeleter::new(CacacheDeleter::new(
                self.core.clone(),
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
