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

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::File;

use super::config::OpfsConfig;
use super::core::OpfsCore;
use super::core::*;
use super::core::*;
use super::deleter::OpfsDeleter;
use super::lister::OpfsLister;
use super::reader::OpfsReadStream;
use super::reader::*;
use super::writer::OpfsWriter;
use opendal_core::raw::*;
use opendal_core::*;

#[doc = include_str!("docs.md")]
#[derive(Default, Debug)]
pub struct OpfsBuilder {
    pub(super) config: OpfsConfig,
}

impl OpfsBuilder {
    /// Set root directory for this backend.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };
        self
    }
}

impl Builder for OpfsBuilder {
    type Config = OpfsConfig;

    fn build(self) -> Result<impl Service> {
        let root = normalize_root(&self.config.root.unwrap_or_default());
        let core = Arc::new(OpfsCore::new(root));
        Ok(OpfsBackend { core })
    }
}

/// OPFS Service backend
#[derive(Debug, Clone)]
pub struct OpfsBackend {
    pub(crate) core: Arc<OpfsCore>,
}

impl Service for OpfsBackend {
    type Reader = oio::StreamReader<OpfsReader>;
    type Writer = OpfsWriter;
    type Lister = OpfsLister;
    type Deleter = oio::OneShotDeleter<OpfsDeleter>;
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.core.info.clone()
    }

    fn capability(&self) -> Capability {
        self.core.capability
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let p = build_abs_path(&self.core.root, path);

        if p.ends_with('/') {
            get_directory_handle(&p, false).await?;
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        // File: get metadata via getFile().
        let handle = get_file_handle(&p, false).await?;
        let file: File = JsFuture::from(handle.get_file())
            .await
            .and_then(JsCast::dyn_into)
            .map_err(parse_js_error)?;

        let mut meta = MetadataBuilder::file(file.size() as u64);
        if let Ok(t) = Timestamp::from_millisecond(file.last_modified() as i64) {
            meta.last_modified(t);
        }

        Ok(RpStat::new(meta.build()))
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<OpfsReader> = {
            Ok(oio::StreamReader::new(OpfsReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn list(&self, _ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        let output: OpfsLister = { Ok(OpfsLister::new(self.core.clone(), path.to_string())) }?;

        Ok(output)
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        debug_assert!(path != "/", "root path should be handled upstream");
        let p = build_abs_path(&self.core.root, path);
        get_directory_handle(&p, true).await?;

        Ok(RpCreateDir::default())
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _args: OpWrite) -> Result<Self::Writer> {
        let output: OpfsWriter = { Ok(OpfsWriter::new(self.core.clone(), path.to_string())) }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<OpfsDeleter> = {
            Ok(oio::OneShotDeleter::new(OpfsDeleter::new(
                self.core.clone(),
            )))
        }?;

        Ok(output)
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
