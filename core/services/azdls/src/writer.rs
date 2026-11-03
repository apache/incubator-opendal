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

use asyncband::once::OnceCell;
use http::StatusCode;

use super::core::FILE;
use super::core::X_MS_VERSION_ID;
use super::core::parse_error;
use super::core::{AzdlsCore, ErrorContext};
use opendal_core::raw::*;
use opendal_core::*;

/// Writer type for azdls: non-append uses PositionWriter, append uses AppendWriter.
pub type AzdlsWriters =
    TwoWays<oio::PositionWriter<AzdlsLazyPositionWriter>, oio::AppendWriter<AzdlsWriter>>;

pub struct AzdlsLazyPositionWriter {
    inner: AzdlsWriter,
    created: OnceCell<()>,
}

impl AzdlsLazyPositionWriter {
    pub fn new(inner: AzdlsWriter) -> Self {
        Self {
            inner,
            created: OnceCell::new(),
        }
    }

    async fn ensure_created(&self) -> Result<()> {
        self.created
            .get_or_try_init(async || self.inner.create_if_needed().await)
            .await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct AzdlsWriter {
    core: Arc<AzdlsCore>,
    ctx: OperationContext,
    op: OpWrite,
    path: String,
}

impl AzdlsWriter {
    pub fn new(core: Arc<AzdlsCore>, ctx: OperationContext, op: OpWrite, path: String) -> Self {
        Self {
            core,
            ctx,
            op,
            path,
        }
    }

    async fn create_if_needed(&self) -> Result<()> {
        let resp = self
            .core
            .azdls_create(&self.ctx, &self.path, FILE, &self.op)
            .await?;
        match resp.status() {
            StatusCode::CREATED | StatusCode::OK => Ok(()),
            _ => {
                let err = parse_error(
                    ErrorContext::new(ServiceOperation("CreateFile"))
                        .with_caller_condition(self.op.is_conditional())
                        .with_if_not_exists(self.op.if_not_exists()),
                    resp,
                )
                .with_operation("Backend::azdls_create_request");
                if err.kind() == ErrorKind::AlreadyExists {
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }

    fn parse_metadata(headers: &http::HeaderMap) -> Result<Metadata> {
        let mut metadata = MetadataBuilder::unknown();

        if let Some(last_modified) = parse_last_modified(headers)? {
            metadata.last_modified(last_modified);
        }
        let etag = parse_etag(headers)?;
        if let Some(etag) = etag {
            metadata.etag(etag);
        }
        let version_id = parse_header_to_str(headers, X_MS_VERSION_ID)?;
        if let Some(version_id) = version_id {
            metadata.version(version_id);
        }

        Ok(metadata.build())
    }
}

impl oio::PositionWrite for AzdlsWriter {
    async fn write_all_at(&self, offset: u64, buf: Buffer) -> Result<()> {
        let size = buf.len() as u64;
        let resp = self
            .core
            .azdls_append(&self.ctx, &self.path, Some(size), offset, false, false, buf)
            .await?;

        match resp.status() {
            StatusCode::OK | StatusCode::ACCEPTED => Ok(()),
            _ => Err(
                parse_error(ErrorContext::new(ServiceOperation("AppendData")), resp)
                    .with_operation("Backend::azdls_append_request"),
            ),
        }
    }

    async fn close(&self, size: u64) -> Result<Metadata> {
        // Flush accumulated appends once.
        let resp = self
            .core
            .azdls_flush(&self.ctx, &self.path, size, true)
            .await?;

        let mut meta = AzdlsWriter::parse_metadata(resp.headers())?.into_builder();
        meta.set_file(size);

        match resp.status() {
            StatusCode::OK | StatusCode::ACCEPTED => Ok(meta.build()),
            _ => Err(
                parse_error(ErrorContext::new(ServiceOperation("FlushData")), resp)
                    .with_operation("Backend::azdls_flush_request"),
            ),
        }
    }

    async fn abort(&self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "Abort is not supported for azdls writer",
        ))
    }
}

impl oio::PositionWrite for AzdlsLazyPositionWriter {
    async fn write_all_at(&self, offset: u64, buf: Buffer) -> Result<()> {
        self.ensure_created().await?;
        self.inner.write_all_at(offset, buf).await
    }

    async fn close(&self, size: u64) -> Result<Metadata> {
        self.ensure_created().await?;
        self.inner.close(size).await
    }

    async fn abort(&self) -> Result<()> {
        self.inner.abort().await
    }
}

impl oio::AppendWrite for AzdlsWriter {
    async fn offset(&self) -> Result<u64> {
        let resp = self
            .core
            .azdls_get_properties(&self.ctx, &self.path, &OpStat::default())
            .await?;

        let status = resp.status();
        let headers = resp.headers();

        match status {
            StatusCode::OK => Ok(parse_content_length(headers)?.unwrap_or_default()),
            StatusCode::NOT_FOUND => Ok(0),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetPathProperties")),
                resp,
            )),
        }
    }

    async fn append(&self, offset: u64, size: u64, body: Buffer) -> Result<Metadata> {
        if offset == 0 {
            // Only create when starting a new file; avoid 404 when appending to a non-existent path.
            self.create_if_needed().await?;
        }

        // append + flush in a single request to minimize roundtrips for append mode.
        let resp = self
            .core
            .azdls_append(&self.ctx, &self.path, Some(size), offset, true, false, body)
            .await?;

        let mut meta = AzdlsWriter::parse_metadata(resp.headers())?.into_builder();
        let md5 = parse_content_md5(resp.headers())?;
        if let Some(md5) = md5 {
            meta.content_md5(md5);
        }
        meta.set_file(offset + size);

        match resp.status() {
            StatusCode::OK | StatusCode::ACCEPTED => Ok(meta.build()),
            _ => Err(
                parse_error(ErrorContext::new(ServiceOperation("AppendData")), resp)
                    .with_operation("Backend::azdls_append_request"),
            ),
        }
    }
}
