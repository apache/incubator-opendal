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
use http::{Request, StatusCode};

use crate::raw::*;
use crate::*;

use super::core::YandexDiskCore;
use super::error::parse_error;

pub type YandexDiskWriters = oio::OneShotWriter<YandexDiskWriter>;

pub struct YandexDiskWriter {
    core: Arc<YandexDiskCore>,
    path: String,
}

impl YandexDiskWriter {
    pub fn new(core: Arc<YandexDiskCore>, path: String) -> Self {
        YandexDiskWriter { core, path }
    }
}

#[async_trait]
impl oio::OneShotWrite for YandexDiskWriter {
    async fn write_once(&self, bs: &dyn oio::WriteBuf) -> Result<()> {
        self.core.ensure_dir_exists(&self.path).await?;

        let upload_url = self.core.get_upload_url(&self.path).await?;

        let bs = bs.bytes(bs.remaining());

        let req = Request::put(upload_url)
            .body(AsyncBody::Bytes(bs))
            .map_err(new_request_build_error)?;

        let resp = self.core.send(req).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED => {
                resp.into_body().consume().await?;
                Ok(())
            }
            _ => Err(parse_error(resp).await?),
        }
    }
}
