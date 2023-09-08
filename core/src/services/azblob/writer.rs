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
use std::task::{Context, Poll};

use async_trait::async_trait;
use http::StatusCode;

use super::core::AzblobCore;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

const X_MS_BLOB_TYPE: &str = "x-ms-blob-type";
const X_MS_BLOB_APPEND_OFFSET: &str = "x-ms-blob-append-offset";

pub struct AzblobWriter {
    core: Arc<AzblobCore>,

    op: OpWrite,
    path: String,

    position: Option<u64>,
}

impl AzblobWriter {
    pub fn new(core: Arc<AzblobCore>, op: OpWrite, path: String) -> Self {
        AzblobWriter {
            core,
            op,
            path,
            position: None,
        }
    }

    async fn write_oneshot(&self, size: u64, body: AsyncBody) -> Result<()> {
        let mut req = self.core.azblob_put_blob_request(
            &self.path,
            Some(size),
            self.op.content_type(),
            self.op.cache_control(),
            body,
        )?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => {
                resp.into_body().consume().await?;
                Ok(())
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn current_position(&mut self) -> Result<Option<u64>> {
        if let Some(v) = self.position {
            return Ok(Some(v));
        }

        // TODO: we should check with current etag to make sure file not changed.
        let resp = self
            .core
            .azblob_get_blob_properties(&self.path, None, None)
            .await?;

        let status = resp.status();

        match status {
            // Just check the blob type.
            // If it is not an appendable blob, return an error.
            // We can not get the append position of the blob here.
            StatusCode::OK => {
                let headers = resp.headers();
                let blob_type = headers.get(X_MS_BLOB_TYPE).and_then(|v| v.to_str().ok());
                if blob_type != Some("AppendBlob") {
                    return Err(Error::new(
                        ErrorKind::ConditionNotMatch,
                        "the blob is not an appendable blob.",
                    ));
                }
                Ok(None)
            }
            // If the blob is not existing, we need to create one.
            StatusCode::NOT_FOUND => {
                let mut req = self.core.azblob_init_appendable_blob_request(
                    &self.path,
                    self.op.content_type(),
                    self.op.cache_control(),
                )?;

                self.core.sign(&mut req).await?;

                let resp = self.core.client.send(req).await?;

                let status = resp.status();
                match status {
                    StatusCode::CREATED => {
                        // do nothing
                    }
                    _ => {
                        return Err(parse_error(resp).await?);
                    }
                }

                self.position = Some(0);
                Ok(Some(0))
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn append_oneshot(&mut self, size: u64, body: AsyncBody) -> Result<()> {
        let _ = self.current_position().await?;

        let mut req =
            self.core
                .azblob_append_blob_request(&self.path, size, self.position, body)?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();
        match status {
            StatusCode::CREATED => {
                let headers = resp.headers();
                let position = headers
                    .get(X_MS_BLOB_APPEND_OFFSET)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                self.position = position.map(|v| v + size);
            }
            _ => {
                return Err(parse_error(resp).await?);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl oio::Write for AzblobWriter {
    fn poll_write(&mut self, _: &mut Context<'_>, _: &dyn oio::WriteBuf) -> Poll<Result<usize>> {
        todo!()
    }

    fn poll_abort(&mut self, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(&mut self, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}
