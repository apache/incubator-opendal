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

use super::ALLUXIO_SCHEME;
use super::config::AlluxioConfig;
use super::core::AlluxioCore;
use super::deleter::AlluxioDeleter;
use super::lister::AlluxioLister;
use super::reader::*;
use super::writer::AlluxioWriter;
use super::writer::AlluxioWriters;
use opendal_core::raw::*;
use opendal_core::*;

/// [Alluxio](https://www.alluxio.io/) services support.
#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct AlluxioBuilder {
    pub(super) config: AlluxioConfig,
}

impl Debug for AlluxioBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlluxioBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl AlluxioBuilder {
    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// endpoint of this backend.
    ///
    /// Endpoint must be full uri, mostly like `http://127.0.0.1:39999`.
    pub fn endpoint(mut self, endpoint: &str) -> Self {
        if !endpoint.is_empty() {
            // Trim trailing `/` so that we can accept `http://127.0.0.1:39999/`
            self.config.endpoint = Some(endpoint.trim_end_matches('/').to_string())
        }

        self
    }
}

impl Builder for AlluxioBuilder {
    type Config = AlluxioConfig;

    /// Builds the backend and returns the result of AlluxioBackend.
    fn build(self) -> Result<impl Service> {
        debug!("backend build started: {:?}", self);

        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        debug!("backend use root {}", root);

        let endpoint = match &self.config.endpoint {
            Some(endpoint) => Ok(endpoint.clone()),
            None => Err(Error::new(ErrorKind::ConfigInvalid, "endpoint is empty")
                .with_operation("Builder::build")
                .with_context("service", ALLUXIO_SCHEME)),
        }?;
        debug!("backend use endpoint {}", endpoint);

        Ok(AlluxioBackend {
            core: Arc::new(AlluxioCore {
                info: ServiceInfo::new(ALLUXIO_SCHEME, &root, ""),
                capability: Capability {
                    stat: true,

                    // FIXME:
                    //
                    // alluxio's read support is not implemented correctly
                    // We need to refactor by use [page_read](https://github.com/Alluxio/alluxio-py/blob/main/alluxio/const.py#L18)
                    read: false,

                    write: true,
                    write_can_multi: true,

                    create_dir: true,
                    delete: true,

                    list: true,

                    shared: true,

                    ..Default::default()
                },
                root,
                endpoint,
            }),
        })
    }
}

#[derive(Debug, Clone)]
pub struct AlluxioBackend {
    pub(crate) core: Arc<AlluxioCore>,
}

impl Service for AlluxioBackend {
    type Reader = oio::StreamReader<AlluxioReader>;
    type Writer = AlluxioWriters;
    type Lister = oio::PageLister<AlluxioLister>;
    type Deleter = oio::OneShotDeleter<AlluxioDeleter>;
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
        self.core.create_dir(ctx, path).await?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, _: OpStat) -> Result<RpStat> {
        let file_info = self.core.get_status(ctx, path).await?;

        Ok(RpStat::new(file_info.try_into()?))
    }
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let output: oio::StreamReader<AlluxioReader> = {
            Ok(oio::StreamReader::new(AlluxioReader::new(
                self.clone(),
                ctx.clone(),
                path,
                args,
            )))
        }?;

        Ok(output)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let output: AlluxioWriters = {
            let w = AlluxioWriter::new(
                self.core.clone(),
                ctx.clone(),
                args.clone(),
                path.to_string(),
            );

            Ok(w)
        }?;

        Ok(output)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let output: oio::OneShotDeleter<AlluxioDeleter> = {
            Ok(oio::OneShotDeleter::new(AlluxioDeleter::new(
                self.core.clone(),
                ctx.clone(),
            )))
        }?;

        Ok(output)
    }

    fn list(&self, ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        let output: oio::PageLister<AlluxioLister> = {
            let l = AlluxioLister::new(self.core.clone(), ctx.clone(), path);
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
        ctx: &OperationContext,
        from: &str,
        to: &str,
        _: OpRename,
    ) -> Result<RpRename> {
        self.core.rename(ctx, from, to).await?;

        Ok(RpRename::default())
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

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_builder_from_map() {
        let mut map = HashMap::new();
        map.insert("root".to_string(), "/".to_string());
        map.insert("endpoint".to_string(), "http://127.0.0.1:39999".to_string());

        let builder = AlluxioConfig::from_iter(map).unwrap();

        assert_eq!(builder.root, Some("/".to_string()));
        assert_eq!(builder.endpoint, Some("http://127.0.0.1:39999".to_string()));
    }

    #[test]
    fn test_builder_build() {
        let builder = AlluxioBuilder::default()
            .root("/root")
            .endpoint("http://127.0.0.1:39999")
            .build();

        assert!(builder.is_ok());
    }
}
