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

use bytes::Buf;
use http::StatusCode;

use super::core::PcloudCore;
use super::error::parse_error;
use super::error::PcloudError;
use crate::raw::*;
use crate::*;

pub type PcloudWriters = oio::OneShotWriter<PcloudWriter>;

pub struct PcloudWriter {
    core: Arc<PcloudCore>,
    path: String,
}

impl PcloudWriter {
    pub fn new(core: Arc<PcloudCore>, path: String) -> Self {
        PcloudWriter { core, path }
    }
}

impl oio::OneShotWrite for PcloudWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        self.core.ensure_dir_exists(&self.path).await?;

        let resp = self.core.upload_file(&self.path, bs).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();
                let resp: PcloudError =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                let result = resp.result;

                if result != 0 {
                    return Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")));
                }

                Ok(Metadata::default())
            }
            _ => Err(parse_error(resp)),
        }
    }
}
