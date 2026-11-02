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

use asyncband::once::OnceCell;

use super::TIKV_SCHEME;
use super::config::TikvConfig;
use super::core::*;
use super::deleter::TikvDeleter;
use super::reader::*;
use super::writer::TikvWriter;
use opendal_core::raw::oio;
use opendal_core::raw::*;
use opendal_core::*;

/// TiKV backend builder
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct TikvBuilder {
    pub(super) config: TikvConfig,
}

impl TikvBuilder {
    /// Set the network address of the TiKV service.
    pub fn endpoints(mut self, endpoints: Vec<String>) -> Self {
        if !endpoints.is_empty() {
            self.config.endpoints = Some(endpoints)
        }
        self
    }

    /// Set the insecure connection to TiKV.
    pub fn insecure(mut self) -> Self {
        self.config.insecure = true;
        self
    }

    /// Set the certificate authority file path.
    pub fn ca_path(mut self, ca_path: &str) -> Self {
        if !ca_path.is_empty() {
            self.config.ca_path = Some(ca_path.to_string())
        }
        self
    }

    /// Set the certificate file path.
    pub fn cert_path(mut self, cert_path: &str) -> Self {
        if !cert_path.is_empty() {
            self.config.cert_path = Some(cert_path.to_string())
        }
        self
    }

    /// Set the key file path.
    pub fn key_path(mut self, key_path: &str) -> Self {
        if !key_path.is_empty() {
            self.config.key_path = Some(key_path.to_string())
        }
        self
    }
}

impl Builder for TikvBuilder {
    type Config = TikvConfig;

    fn build(self) -> Result<impl Service> {
        let endpoints = self.config.endpoints.ok_or_else(|| {
            Error::new(
                ErrorKind::ConfigInvalid,
                "endpoints is required but not set",
            )
            .with_context("service", TIKV_SCHEME)
        })?;

        if self.config.insecure
            && (self.config.ca_path.is_some()
                || self.config.key_path.is_some()
                || self.config.cert_path.is_some())
        {
            Err(
                Error::new(ErrorKind::ConfigInvalid, "invalid tls configuration")
                    .with_context("service", TIKV_SCHEME)
                    .with_context("endpoints", format!("{endpoints:?}")),
            )?;
        }

        Ok(TikvBackend::new(TikvCore {
            client: OnceCell::new(),
            endpoints,
            insecure: self.config.insecure,
            ca_path: self.config.ca_path.clone(),
            cert_path: self.config.cert_path.clone(),
            key_path: self.config.key_path.clone(),
        }))
    }
}

/// Backend for TiKV service
#[derive(Clone, Debug)]
pub struct TikvBackend {
    pub(crate) core: Arc<TikvCore>,
    pub(crate) root: String,
    pub(crate) info: ServiceInfo,
    pub(crate) capability: Capability,
}

impl TikvBackend {
    fn new(core: TikvCore) -> Self {
        let info = ServiceInfo::new(TIKV_SCHEME, "/", "TiKV");
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
}

impl Service for TikvBackend {
    type Reader = oio::StreamReader<TikvReader>;
    type Writer = TikvWriter;
    type Lister = ();
    type Deleter = oio::OneShotDeleter<TikvDeleter>;
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
                None => Err(Error::new(ErrorKind::NotFound, "kv not found in tikv")),
            }
        }
    }
    fn read(&self, _ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<TikvReader> = {
            Ok(oio::StreamReader::new(TikvReader::new(
                self.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, _ctx: &OperationContext, path: &str, _: OpWrite) -> Result<Self::Writer> {
        let output: TikvWriter = {
            let p = build_abs_path(&self.root, path);
            Ok(TikvWriter::new(self.core.clone(), p))
        }?;

        Ok(output)
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<TikvDeleter> = {
            Ok(oio::OneShotDeleter::new(TikvDeleter::new(
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
