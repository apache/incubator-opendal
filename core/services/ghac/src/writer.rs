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

use std::str::FromStr;
use std::sync::Arc;

use super::core::parse_error;
use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;
use opendal_service_azblob::core::AzblobCore;
use opendal_service_azblob::writer::AzblobWriter;
use reqsign_azure_storage::RequestSigner;
use reqsign_azure_storage::StaticCredentialProvider;
use reqsign_core::Context;
use reqsign_core::Signer;

pub struct GhacWriter(pub TwoWays<GhacWriterV1, GhacWriterV2>);

impl GhacWriter {
    /// TODO: maybe we can move the signed url logic to azblob service instead.
    pub fn new(
        core: Arc<GhacCore>,
        ctx: OperationContext,
        executor: Executor,
        write_path: String,
        url: String,
    ) -> Result<Self> {
        match core.service_version {
            GhacVersion::V1 => Ok(GhacWriter(TwoWays::One(GhacWriterV1 {
                core,
                ctx,
                path: write_path,
                url,
                size: 0,
            }))),
            GhacVersion::V2 => {
                let uri = http::Uri::from_str(&url)
                    .map_err(new_http_uri_invalid_error)?
                    .into_parts();
                let (Some(scheme), Some(authority), Some(pq)) =
                    (uri.scheme, uri.authority, uri.path_and_query)
                else {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        "ghac returns invalid signed url",
                    )
                    .with_context("url", &url));
                };
                let endpoint = format!("{scheme}://{authority}");
                let Some((container, path)) = pq.path().trim_matches('/').split_once("/") else {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        "ghac returns invalid signed url that bucket or path is missing",
                    )
                    .with_context("url", &url));
                };
                let Some(query) = pq.query() else {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        "ghac returns invalid signed url that sas is missing",
                    )
                    .with_context("url", &url));
                };
                let signer = Signer::new(
                    Context::new(),
                    StaticCredentialProvider::new_sas_token(query),
                    RequestSigner::new(),
                );
                let azure_core = Arc::new(AzblobCore {
                    info: ServiceInfo::new("azblob", "/", container),
                    capability: Capability {
                        stat: true,
                        stat_with_if_match: true,
                        stat_with_if_none_match: true,

                        read: true,

                        read_with_if_match: true,
                        read_with_if_none_match: true,
                        read_with_override_content_disposition: true,
                        read_with_if_modified_since: true,
                        read_with_if_unmodified_since: true,

                        write: true,
                        write_can_append: true,
                        write_can_empty: true,
                        write_can_multi: true,
                        write_with_cache_control: true,
                        write_with_content_type: true,
                        write_with_if_not_exists: true,
                        write_with_if_none_match: true,
                        write_with_user_metadata: true,

                        copy: true,

                        list: true,
                        list_with_recursive: true,

                        shared: true,

                        ..Default::default()
                    },
                    container: container.to_string(),
                    root: "/".to_string(),
                    endpoint,
                    encryption_key: None,
                    encryption_key_sha256: None,
                    encryption_algorithm: None,
                    skip_signature: false,
                    signer,
                });
                let w = AzblobWriter::new(
                    azure_core,
                    ctx.clone(),
                    OpWrite::default(),
                    path.to_string(),
                );
                let writer = oio::BlockWriter::new(executor, w, 4);
                Ok(GhacWriter(TwoWays::Two(GhacWriterV2 {
                    core,
                    ctx,
                    writer,
                    path: write_path,
                    url,
                    size: 0,
                })))
            }
        }
    }
}

pub struct GhacLazyWriter {
    core: Arc<GhacCore>,
    ctx: OperationContext,
    executor: Executor,
    path: String,
    inner: Option<GhacWriter>,
}

impl GhacLazyWriter {
    pub fn new(
        core: Arc<GhacCore>,
        ctx: OperationContext,
        executor: Executor,
        path: String,
    ) -> Self {
        Self {
            core,
            ctx,
            executor,
            path,
            inner: None,
        }
    }

    async fn inner(&mut self) -> Result<&mut GhacWriter> {
        if self.inner.is_none() {
            let url = self.core.ghac_get_upload_url(&self.ctx, &self.path).await?;
            self.inner = Some(GhacWriter::new(
                self.core.clone(),
                self.ctx.clone(),
                self.executor.clone(),
                self.path.clone(),
                url,
            )?);
        }

        Ok(self.inner.as_mut().expect("writer must be initialized"))
    }
}

impl oio::Write for GhacLazyWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner().await?.write(bs).await
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner().await?.abort().await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner().await?.close().await
    }
}

impl oio::Write for GhacWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.0.write(bs).await
    }

    async fn abort(&mut self) -> Result<()> {
        self.0.abort().await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.0.close().await
    }
}

pub struct GhacWriterV1 {
    core: Arc<GhacCore>,
    ctx: OperationContext,

    path: String,
    url: String,
    size: u64,
}

impl oio::Write for GhacWriterV1 {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let size = bs.len() as u64;
        let offset = self.size;

        let resp = self
            .core
            .ghac_v1_write(&self.ctx, &self.url, size, offset, bs)
            .await?;
        if !resp.status().is_success() {
            return Err(
                parse_error(ErrorContext::new(ServiceOperation("UploadChunk")), resp)
                    .with_operation("Backend::ghac_upload"),
            );
        }
        self.size += size;
        Ok(())
    }

    async fn abort(&mut self) -> Result<()> {
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.core
            .ghac_finalize_upload(&self.ctx, &self.path, &self.url, self.size)
            .await?;
        Ok({
            let mut metadata = MetadataBuilder::unknown();
            metadata.set_file(self.size);
            metadata.build()
        })
    }
}

pub struct GhacWriterV2 {
    core: Arc<GhacCore>,
    ctx: OperationContext,
    writer: oio::BlockWriter<AzblobWriter>,

    path: String,
    url: String,
    size: u64,
}

impl oio::Write for GhacWriterV2 {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let size = bs.len() as u64;

        self.writer.write(bs).await?;
        self.size += size;
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.writer.close().await?;
        self.core
            .ghac_finalize_upload(&self.ctx, &self.path, &self.url, self.size)
            .await?;
        Ok({
            let mut metadata = MetadataBuilder::unknown();
            metadata.set_file(self.size);
            metadata.build()
        })
    }

    async fn abort(&mut self) -> Result<()> {
        Ok(())
    }
}
