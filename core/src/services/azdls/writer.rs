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

use super::core::AzdlsCore;
use super::error::parse_error;
use crate::raw::*;
use crate::*;

pub type AzdlsWriters = TwoWays<oio::OneShotWriter<AzdlsWriter>, oio::AppendWriter<AzdlsWriter>>;

pub struct AzdlsWriter {
    core: Arc<AzdlsCore>,

    op: OpWrite,
    path: String,
}

impl AzdlsWriter {
    pub fn new(core: Arc<AzdlsCore>, op: OpWrite, path: String) -> Self {
        AzdlsWriter { core, op, path }
    }
}

impl oio::OneShotWrite for AzdlsWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let mut req =
            self.core
                .azdls_create_request(&self.path, "file", &self.op, Buffer::new())?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();
        match status {
            StatusCode::CREATED | StatusCode::OK => {}
            _ => {
                return Err(parse_error(resp).with_operation("Backend::azdls_create_request"));
            }
        }

        let mut req = self
            .core
            .azdls_update_request(&self.path, Some(bs.len() as u64), 0, bs)?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::ACCEPTED => Ok(Metadata::default()),
            _ => Err(parse_error(resp).with_operation("Backend::azdls_update_request")),
        }
    }
}

impl oio::AppendWrite for AzdlsWriter {
    async fn offset(&self) -> Result<u64> {
        let resp = self.core.azdls_get_properties(&self.path).await?;

        let status = resp.status();
        let headers = resp.headers();

        match status {
            StatusCode::OK => Ok(parse_content_length(headers)?.unwrap_or_default()),
            StatusCode::NOT_FOUND => Ok(0),
            _ => Err(parse_error(resp)),
        }
    }

    async fn append(&self, offset: u64, size: u64, body: Buffer) -> Result<()> {
        if offset == 0 {
            let mut req =
                self.core
                    .azdls_create_request(&self.path, "file", &self.op, Buffer::new())?;

            self.core.sign(&mut req).await?;

            let resp = self.core.send(req).await?;

            let status = resp.status();
            match status {
                StatusCode::CREATED | StatusCode::OK => {}
                _ => {
                    return Err(parse_error(resp).with_operation("Backend::azdls_create_request"));
                }
            }
        }

        let mut req = self
            .core
            .azdls_update_request(&self.path, Some(size), offset, body)?;

        self.core.sign(&mut req).await?;

        let resp = self.core.send(req).await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::ACCEPTED => Ok(()),
            _ => Err(parse_error(resp).with_operation("Backend::azdls_update_request")),
        }
    }
}
