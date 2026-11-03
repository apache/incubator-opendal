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

use std::io::Cursor;
use std::sync::Arc;

use compio::dispatcher::Dispatcher;
use compio::fs::OpenOptions;

use super::COMPFS_SCHEME;
use super::config::CompfsConfig;
use super::core::CompfsCore;
use super::deleter::CompfsDeleter;
use super::reader::*;
use opendal_core::raw::*;
use opendal_core::*;

/// [`compio`]-based file system support.
#[derive(Debug, Default)]
pub struct CompfsBuilder {
    pub(super) config: CompfsConfig,
}

impl CompfsBuilder {
    /// Set root for Compfs
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }
}

impl Builder for CompfsBuilder {
    type Config = CompfsConfig;

    fn build(self) -> Result<impl Service> {
        let root = match self.config.root {
            Some(root) => Ok(root),
            None => Err(Error::new(
                ErrorKind::ConfigInvalid,
                "root is not specified",
            )),
        }?;

        // If root dir does not exist, we must create it.
        if let Err(e) = std::fs::metadata(&root)
            && e.kind() == std::io::ErrorKind::NotFound
        {
            std::fs::create_dir_all(&root).map_err(|e| {
                Error::new(ErrorKind::Unexpected, "create root dir failed")
                    .with_operation("Builder::build")
                    .with_context("root", root.as_str())
                    .set_source(e)
            })?;
        }

        let dispatcher = Dispatcher::new().map_err(|_| {
            Error::new(
                ErrorKind::Unexpected,
                "failed to initiate compio dispatcher",
            )
        })?;
        let core = CompfsCore {
            info: ServiceInfo::new(COMPFS_SCHEME, &root, ""),
            capability: Capability {
                stat: true,

                read: true,

                write: true,
                write_can_empty: true,
                write_can_multi: true,
                create_dir: true,
                delete: true,

                list: true,

                copy: true,
                rename: true,

                shared: true,

                ..Default::default()
            },
            root: root.into(),
            dispatcher,
            buf_pool: oio::PooledBuf::new(16),
        };
        Ok(CompfsBackend {
            core: Arc::new(core),
        })
    }
}

#[derive(Clone, Debug)]
pub struct CompfsBackend {
    pub(crate) core: Arc<CompfsCore>,
}

impl Service for CompfsBackend {
    type Reader = oio::PositionReader<CompfsReader>;
    type Writer = CompfsLazyWriter;
    type Lister = CompfsLazyLister;
    type Deleter = oio::OneShotDeleter<CompfsDeleter>;
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
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let path = self.core.prepare_path(path)?;

        self.core
            .exec(move || async move { compio::fs::create_dir_all(path).await })
            .await?;

        Ok(RpCreateDir::default())
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let path = self.core.prepare_path(path)?;
        let meta = self
            .core
            .exec(move || async move { compio::fs::metadata(path).await })
            .await?;
        let ty = meta.file_type();
        let mode = if ty.is_dir() {
            EntryMode::DIR
        } else if ty.is_file() {
            EntryMode::FILE
        } else {
            EntryMode::Unknown
        };
        let last_mod = Timestamp::try_from(meta.modified().map_err(new_std_io_error)?)?;
        let mut ret = match mode {
            EntryMode::FILE => MetadataBuilder::file(meta.len()),
            EntryMode::DIR => MetadataBuilder::dir(),
            EntryMode::Unknown => MetadataBuilder::unknown(),
        };
        ret.last_modified(last_mod);
        Ok(RpStat::new(ret.build()))
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<CompfsDeleter> = {
            Ok(oio::OneShotDeleter::new(CompfsDeleter::new(
                self.core.clone(),
            )))
        }?;

        Ok(output)
    }

    fn copy(
        &self,
        _ctx: &OperationContext,
        from: &str,
        to: &str,
        _: OpCopy,
    ) -> Result<Self::Copier> {
        let core = self.core.clone();
        let from = self.core.prepare_path(from)?;
        let to = self.core.prepare_path(to)?;

        Ok(oio::OneShotCopier::new(async move {
            core.exec(move || async move {
                let from = OpenOptions::new().read(true).open(from).await?;
                if let Some(parent) = to.parent() {
                    compio::fs::create_dir_all(parent).await?;
                }
                let to = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(to)
                    .await?;

                let (mut from, mut to) = (Cursor::new(from), Cursor::new(to));
                let size = compio::io::copy(&mut from, &mut to).await?;

                let metadata = MetadataBuilder::file(size);
                Ok(metadata.build())
            })
            .await
        }))
    }

    async fn rename(
        &self,
        _ctx: &OperationContext,
        from: &str,
        to: &str,
        _: OpRename,
    ) -> Result<RpRename> {
        let from = self.core.prepare_path(from)?;
        let to = self.core.prepare_path(to)?;

        self.core
            .exec(move || async move {
                if let Some(parent) = to.parent() {
                    compio::fs::create_dir_all(parent).await?;
                }
                compio::fs::rename(from, to).await
            })
            .await?;

        Ok(RpRename::default())
    }
    fn read(&self, _ctx: &OperationContext, path: &str, _: OpRead) -> Result<Self::Reader> {
        Ok(oio::PositionReader::new(CompfsReader::new(
            self.core.clone(),
            self.core.prepare_path(path)?,
        )))
    }

    fn write(&self, _ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        Ok(CompfsLazyWriter::new(
            self.core.clone(),
            self.core.prepare_path(path)?,
            args,
        ))
    }

    fn list(&self, _ctx: &OperationContext, path: &str, _: OpList) -> Result<Self::Lister> {
        Ok(CompfsLazyLister::new(
            self.core.clone(),
            self.core.prepare_path(path)?,
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
