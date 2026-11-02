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

use http::StatusCode;
use log::debug;
use prost::Message;

use super::reader::*;
use crate::IPFS_SCHEME;
use crate::config::IpfsConfig;
use crate::core::parse_error;
use crate::core::{ErrorContext, IpfsCore};
use crate::ipld::PBNode;
use opendal_core::raw::*;
use opendal_core::*;

/// IPFS file system support based on [IPFS HTTP Gateway](https://docs.ipfs.tech/concepts/ipfs-gateway/).
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct IpfsBuilder {
    pub(super) config: IpfsConfig,
}

impl Debug for IpfsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpfsBuilder")
            .field("config", &self.config)
            .finish()
    }
}

impl IpfsBuilder {
    /// Set root of ipfs backend.
    ///
    /// Root must be a valid ipfs address like the following:
    ///
    /// - `/ipfs/QmPpCt1aYGb9JWJRmXRUnmJtVgeFFTJGzWFYEEX7bo9zGJ/` (IPFS with CID v0)
    /// - `/ipfs/bafybeibozpulxtpv5nhfa2ue3dcjx23ndh3gwr5vwllk7ptoyfwnfjjr4q/` (IPFS with  CID v1)
    /// - `/ipns/opendal.apache.org/` (IPNS)
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set endpoint if ipfs backend.
    ///
    /// Endpoint must be a valid ipfs gateway which passed the [IPFS Gateway Checker](https://ipfs.github.io/public-gateway-checker/)
    ///
    /// Popular choices including:
    ///
    /// - `https://ipfs.io`
    /// - `https://w3s.link`
    /// - `https://dweb.link`
    /// - `https://cloudflare-ipfs.com`
    /// - `http://127.0.0.1:8080` (ipfs daemon in local)
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            // Trim trailing `/` so that we can accept `http://127.0.0.1:9000/`
            self.config.endpoint = Some(endpoint.trim_end_matches('/').to_string());
        }

        self
    }
}

impl Builder for IpfsBuilder {
    type Config = IpfsConfig;

    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.unwrap_or_default());
        if !root.starts_with("/ipfs/") && !root.starts_with("/ipns/") {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "root must start with /ipfs/ or /ipns/",
            )
            .with_context("service", IPFS_SCHEME)
            .with_context("root", &root));
        }
        debug!("backend use root {root}");

        let endpoint = match &self.config.endpoint {
            Some(endpoint) => Ok(endpoint.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_context("service", IPFS_SCHEME)
                .with_context("root", &root)),
        }?;
        debug!("backend use endpoint {}", endpoint);

        let info = ServiceInfo::new(IPFS_SCHEME, &root, "");
        let capability = Capability {
            stat: true,

            read: true,
            read_with_suffix: true,

            list: true,

            shared: true,

            ..Default::default()
        };

        let accessor_info = info;
        let core = Arc::new(IpfsCore {
            info: accessor_info,
            capability,
            root,
            endpoint,
        });

        Ok(IpfsBackend { core })
    }
}

/// Backend for IPFS.
#[derive(Clone, Debug)]
pub struct IpfsBackend {
    pub(crate) core: Arc<IpfsCore>,
}

impl Service for IpfsBackend {
    type Reader = oio::StreamReader<IpfsReader>;
    type Writer = ();
    type Lister = oio::PageLister<DirStream>;
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

    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let metadata = self.core.ipfs_stat(ctx, path).await?;
        Ok(RpStat::new(metadata))
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<IpfsReader> = {
            Ok(oio::StreamReader::new(IpfsReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, _path: &str, _args: OpWrite) -> Result<Self::Writer> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn list(&self, ctx: &OperationContext, path: &str, _: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<DirStream> = {
            let l = DirStream::new(self.core.clone(), ctx.clone(), path);
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

pub struct DirStream {
    core: Arc<IpfsCore>,
    ctx: OperationContext,
    path: String,
}

impl DirStream {
    fn new(core: Arc<IpfsCore>, ctx: OperationContext, path: &str) -> Self {
        Self {
            core,
            ctx,
            path: path.to_string(),
        }
    }
}

impl oio::PageList for DirStream {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self.core.ipfs_list(&self.ctx, &self.path).await?;

        if resp.status() != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("List")),
                resp,
            ));
        }

        let bs = resp.into_body();
        let pb_node = PBNode::decode(bs).map_err(|e| {
            Error::new(ErrorKind::Unexpected, "deserialize protobuf from response").set_source(e)
        })?;

        let names = pb_node
            .links
            .into_iter()
            .map(|v| v.name.unwrap())
            .collect::<Vec<String>>();

        for name in names {
            let child = if self.path == "/" {
                name
            } else {
                format!("{}{}", self.path, name)
            };

            let meta = self.core.ipfs_stat(&self.ctx, &child).await?;

            let child = if meta.mode().is_dir() {
                format!("{child}/")
            } else {
                child
            };

            ctx.entries.push_back(oio::Entry::new(&child, meta))
        }

        ctx.done = true;
        Ok(())
    }
}
