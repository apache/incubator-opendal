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

use http::StatusCode;
use uuid::Uuid;

use super::core::AzblobCore;
use super::core::ErrorContext;
use super::core::constants::X_MS_VERSION_ID;
use super::core::parse_error;
use opendal_core::raw::*;
use opendal_core::*;

const X_MS_BLOB_TYPE: &str = "x-ms-blob-type";

pub type AzblobWriters = TwoWays<oio::BlockWriter<AzblobWriter>, oio::AppendWriter<AzblobWriter>>;

pub struct AzblobWriter {
    core: Arc<AzblobCore>,
    ctx: OperationContext,

    op: OpWrite,
    path: String,
}

impl AzblobWriter {
    pub fn new(core: Arc<AzblobCore>, ctx: OperationContext, op: OpWrite, path: String) -> Self {
        AzblobWriter {
            core,
            ctx,
            op,
            path,
        }
    }

    // skip extracting `content-md5` here, as it pertains to the content of the request rather than
    // the content of the block itself for the `append` and `complete put block list` operations.
    pub(crate) fn parse_metadata(headers: &http::HeaderMap) -> Result<Metadata> {
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

    fn error_context(&self, service_operation: ServiceOperation) -> ErrorContext {
        ErrorContext::new(service_operation)
            .with_caller_condition(self.op.is_conditional())
            .with_if_not_exists(self.op.if_not_exists())
    }
}

impl oio::AppendWrite for AzblobWriter {
    async fn offset(&self) -> Result<u64> {
        let resp = self
            .core
            .azblob_get_blob_properties(&self.ctx, &self.path, &OpStat::default())
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                if self.op.if_not_exists() {
                    return Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        "the blob already exists",
                    ));
                }
                let headers = resp.headers();
                let blob_type = headers.get(X_MS_BLOB_TYPE).and_then(|v| v.to_str().ok());
                if blob_type != Some("AppendBlob") {
                    return Err(Error::new(
                        ErrorKind::Conflict,
                        "the blob is not an appendable blob.",
                    ));
                }

                Ok(parse_content_length(headers)?.unwrap_or_default())
            }
            StatusCode::NOT_FOUND => {
                let resp = self
                    .core
                    .azblob_init_appendable_blob(&self.ctx, &self.path, &self.op)
                    .await?;

                let status = resp.status();
                match status {
                    StatusCode::CREATED => {
                        // do nothing
                    }
                    _ => {
                        return Err(parse_error(
                            ErrorContext::new(ServiceOperation("PutBlob"))
                                .with_if_not_exists(self.op.if_not_exists())
                                .with_append_blob_initialization(true),
                            resp,
                        ));
                    }
                }
                Ok(0)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("GetBlobProperties")),
                resp,
            )),
        }
    }

    async fn append(&self, offset: u64, size: u64, body: Buffer) -> Result<Metadata> {
        let resp = self
            .core
            .azblob_append_blob(&self.ctx, &self.path, offset, size, body)
            .await?;

        let meta = AzblobWriter::parse_metadata(resp.headers())?;
        let status = resp.status();
        match status {
            StatusCode::CREATED => Ok(meta),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("AppendBlock")),
                resp,
            )),
        }
    }
}

impl oio::BlockWrite for AzblobWriter {
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        let resp = self
            .core
            .azblob_put_blob(&self.ctx, &self.path, Some(size), &self.op, body)
            .await?;

        let status = resp.status();

        let mut meta = AzblobWriter::parse_metadata(resp.headers())?.into_builder();
        let md5 = parse_content_md5(resp.headers())?;
        if let Some(md5) = md5 {
            meta.content_md5(md5);
        }
        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(meta.build()),
            _ => Err(parse_error(
                self.error_context(ServiceOperation("PutBlob")),
                resp,
            )),
        }
    }

    async fn write_block(&self, block_id: Uuid, size: u64, body: Buffer) -> Result<()> {
        let resp = self
            .core
            .azblob_put_block(&self.ctx, &self.path, block_id, Some(size), &self.op, body)
            .await?;

        let status = resp.status();
        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(()),
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("PutBlock")),
                resp,
            )),
        }
    }

    async fn complete_block(&self, block_ids: Vec<Uuid>) -> Result<Metadata> {
        let resp = self
            .core
            .azblob_complete_put_block_list(&self.ctx, &self.path, block_ids, &self.op)
            .await?;

        let meta = AzblobWriter::parse_metadata(resp.headers())?;
        let status = resp.status();
        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(meta),
            _ => Err(parse_error(
                self.error_context(ServiceOperation("PutBlockList")),
                resp,
            )),
        }
    }

    async fn abort_block(&self, _block_ids: Vec<Uuid>) -> Result<()> {
        // refer to https://learn.microsoft.com/en-us/rest/api/storageservices/put-block-list?tabs=microsoft-entra-id
        // Any uncommitted blocks are garbage collected if there are no successful calls to Put Block or Put Block List on the blob within a week.
        // If Put Blob is called on the blob, any uncommitted blocks are garbage collected.
        Ok(())
    }
}
