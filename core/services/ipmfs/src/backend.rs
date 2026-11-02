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

use bytes::Buf;
use http::StatusCode;
use serde::Deserialize;

use super::core::parse_error;
use super::core::{ErrorContext, IpmfsCore};
use super::deleter::IpmfsDeleter;
use super::lister::IpmfsLister;
use super::reader::*;
use super::writer::IpmfsWriter;
use opendal_core::raw::*;
use opendal_core::*;

/// IPFS Mutable File System (IPMFS) backend.
#[doc = include_str!("docs.md")]
use std::fmt::Debug;

use log::debug;

use super::IPMFS_SCHEME;
use super::config::IpmfsConfig;

/// IPFS file system support based on [IPFS MFS](https://docs.ipfs.tech/concepts/file-systems/) API.
///
/// # Capabilities
///
/// This service can be used to:
///
/// - [x] read
/// - [x] write
/// - [x] list
/// - [ ] presign
/// - [ ] blocking
///
/// # Configuration
///
/// - `root`: Set the work directory for backend
/// - `endpoint`: Customizable endpoint setting
///
/// You can refer to [`IpmfsBuilder`]'s docs for more information
///
/// # Example
///
/// ## Via Builder
///
/// ```rust,no_run
/// use opendal_core::Operator;
/// use opendal_core::Result;
/// use opendal_service_ipmfs::Ipmfs;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut builder = Ipmfs::default()
///         .endpoint("http://127.0.0.1:5001");
///
///     let op: Operator = Operator::new(builder)?;
///     Ok(())
/// }
/// ```
#[derive(Default)]
pub struct IpmfsBuilder {
    pub(super) config: IpmfsConfig,
}

impl Debug for IpmfsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpmfsBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl IpmfsBuilder {
    /// Set root for ipfs.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set endpoint for ipfs.
    ///
    /// Default: http://localhost:5001
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.config.endpoint = if endpoint.is_empty() {
            None
        } else {
            Some(endpoint.to_string())
        };
        self
    }
}

impl Builder for IpmfsBuilder {
    type Config = IpmfsConfig;

    fn build(self) -> Result<impl Service> {
        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {root}");

        let endpoint = self
            .config
            .endpoint
            .clone()
            .unwrap_or_else(|| "http://localhost:5001".to_string());

        let info = ServiceInfo::new(IPMFS_SCHEME, &root, "");
        let capability = Capability {
            stat: true,

            read: true,

            write: true,
            delete: true,

            list: true,

            shared: true,

            ..Default::default()
        };

        let accessor_info = info;
        let core = Arc::new(IpmfsCore {
            info: accessor_info,
            capability,
            root: root.to_string(),
            endpoint: endpoint.to_string(),
        });

        Ok(IpmfsBackend { core })
    }
}

#[derive(Clone, Debug)]
pub struct IpmfsBackend {
    pub core: Arc<IpmfsCore>,
}

impl Service for IpmfsBackend {
    type Reader = oio::StreamReader<IpmfsReader>;
    type Writer = oio::OneShotWriter<IpmfsWriter>;
    type Lister = oio::PageLister<IpmfsLister>;
    type Deleter = oio::OneShotDeleter<IpmfsDeleter>;
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
        ctx: &OperationContext,
        path: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let resp = self.core.ipmfs_mkdir(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(RpCreateDir::default()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("FilesMkdir")),
                resp,
            )),
        }
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        // Stat root always returns a DIR.
        if path == "/" {
            return Ok(RpStat::new(MetadataBuilder::dir().build()));
        }

        let resp = self.core.ipmfs_stat(ctx, path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();

                let res: IpfsStatResponse =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

                let mode = match res.file_type.as_str() {
                    "file" => EntryMode::FILE,
                    "directory" => EntryMode::DIR,
                    _ => EntryMode::Unknown,
                };

                let meta = match mode {
                    EntryMode::FILE => MetadataBuilder::file(res.size),
                    EntryMode::DIR => MetadataBuilder::dir(),
                    EntryMode::Unknown => MetadataBuilder::unknown(),
                };

                Ok(RpStat::new(meta.build()))
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("FilesStat")),
                resp,
            )),
        }
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<IpmfsReader> = {
            Ok(oio::StreamReader::new(IpmfsReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: oio::OneShotWriter<IpmfsWriter> = {
            Ok(oio::OneShotWriter::new(IpmfsWriter::new(
                self.core.clone(),
                ctx.clone(),
                path.to_string(),
            )))
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<IpmfsDeleter> = {
            Ok(oio::OneShotDeleter::new(IpmfsDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, _: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<IpmfsLister> = {
            let l = IpmfsLister::new(self.core.clone(), ctx.clone(), &self.core.root, path);
            Ok(oio::PageLister::new(l))
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

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct IpfsStatResponse {
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "Type")]
    file_type: String,
}
