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
use super::error::parse_error;
use crate::raw::*;
use crate::*;

const X_MS_BLOB_TYPE: &str = "x-ms-blob-type";

pub type AzblobWriters = TwoWays<oio::BlockWriter<AzblobWriter>, oio::AppendWriter<AzblobWriter>>;

pub struct AzblobWriter {
    core: Arc<AzblobCore>,

    op: OpWrite,
    path: String,
}

impl AzblobWriter {
    pub fn new(core: Arc<AzblobCore>, op: OpWrite, path: String) -> Self {
        AzblobWriter { core, op, path }
    }
}

impl oio::AppendWrite for AzblobWriter {
    async fn offset(&self) -> Result<u64> {
        let resp = self
            .core
            .azblob_get_blob_properties(&self.path, &OpStat::default())
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let headers = resp.headers();
                let blob_type = headers.get(X_MS_BLOB_TYPE).and_then(|v| v.to_str().ok());
                if blob_type != Some("AppendBlob") {
                    return Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        "the blob is not an appendable blob.",
                    ));
                }

                Ok(parse_content_length(headers)?.unwrap_or_default())
            }
            StatusCode::NOT_FOUND => {
                let mut req = self
                    .core
                    .azblob_init_appendable_blob_request(&self.path, &self.op)?;

                self.core.sign(&mut req).await?;

                let resp = self.core.client.send(req).await?;

                let status = resp.status();
                match status {
                    StatusCode::CREATED => {
                        // do nothing
                    }
                    _ => {
                        return Err(parse_error(resp));
                    }
                }
                Ok(0)
            }
            _ => Err(parse_error(resp)),
        }
    }

    async fn append(&self, offset: u64, size: u64, body: Buffer) -> Result<()> {
        let mut req = self
            .core
            .azblob_append_blob_request(&self.path, offset, size, body)?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();
        match status {
            StatusCode::CREATED => Ok(()),
            _ => Err(parse_error(resp)),
        }
    }
}

impl oio::BlockWrite for AzblobWriter {
    async fn write_once(&self, size: u64, body: Buffer) -> Result<Metadata> {
        let mut req: http::Request<Buffer> =
            self.core
                .azblob_put_blob_request(&self.path, Some(size), &self.op, body)?;
        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn write_block(&self, block_id: Uuid, size: u64, body: Buffer) -> Result<()> {
        let resp = self
            .core
            .azblob_put_block(&self.path, block_id, Some(size), &self.op, body)
            .await?;

        let status = resp.status();
        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn complete_block(&self, block_ids: Vec<Uuid>) -> Result<Metadata> {
        let resp = self
            .core
            .azblob_complete_put_block_list(&self.path, block_ids, &self.op)
            .await?;

        let status = resp.status();
        match status {
            StatusCode::CREATED | StatusCode::OK => Ok(Metadata::default()),
            _ => Err(parse_error(resp)),
        }
    }

    async fn abort_block(&self, _block_ids: Vec<Uuid>) -> Result<()> {
        // refer to https://learn.microsoft.com/en-us/rest/api/storageservices/put-block-list?tabs=microsoft-entra-id
        // Any uncommitted blocks are garbage collected if there are no successful calls to Put Block or Put Block List on the blob within a week.
        // If Put Blob is called on the blob, any uncommitted blocks are garbage collected.
        Ok(())
    }
}
