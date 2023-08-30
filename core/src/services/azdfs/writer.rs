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

use async_trait::async_trait;
use http::StatusCode;

use super::core::AzdfsCore;
use super::error::parse_error;
use crate::raw::oio::{Stream, Streamer};
use crate::raw::*;
use crate::*;

pub type AzdfsWriters = oio::OneShotWriter<AzdfsWriter>;

pub struct AzdfsWriter {
    core: Arc<AzdfsCore>,

    path: String,
    op: OpWrite,
}

impl AzdfsWriter {
    pub fn new(core: Arc<AzdfsCore>, path: &str, op: OpWrite) -> Self {
        AzdfsWriter {
            core,
            path: path.to_string(),
            op,
        }
    }
}

#[async_trait]
impl oio::OneShotWrite for AzdfsWriter {
    async fn write_once(&self, stream: Streamer) -> Result<()> {
        let mut req = self.core.azdfs_create_request(
            &self.path,
            "file",
            self.op.content_type(),
            self.op.content_disposition(),
            AsyncBody::Empty,
        )?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();
        match status {
            StatusCode::CREATED | StatusCode::OK => {
                resp.into_body().consume().await?;
            }
            _ => {
                return Err(parse_error(resp)
                    .await?
                    .with_operation("Backend::azdfs_create_request"));
            }
        }

        let mut req = self.core.azdfs_update_request(
            &self.path,
            Some(stream.size()),
            AsyncBody::Stream(stream),
        )?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::ACCEPTED => {
                resp.into_body().consume().await?;
                Ok(())
            }
            _ => Err(parse_error(resp)
                .await?
                .with_operation("Backend::azdfs_update_request")),
        }
    }
}
