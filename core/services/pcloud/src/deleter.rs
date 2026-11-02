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
use opendal_core::raw::*;
use opendal_core::*;

use super::core::PcloudError;
use super::core::parse_error;
use super::core::*;

pub struct PcloudDeleter {
    core: Arc<PcloudCore>,
    ctx: OperationContext,
}

impl PcloudDeleter {
    pub fn new(core: Arc<PcloudCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }
}

impl oio::OneShotDelete for PcloudDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let resp = if path.ends_with('/') {
            self.core.delete_folder(&self.ctx, &path).await?
        } else {
            self.core.delete_file(&self.ctx, &path).await?
        };

        let status = resp.status();

        match status {
            StatusCode::OK => {
                let bs = resp.into_body();
                let resp: PcloudError =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                let result = resp.result;

                // pCloud returns 2005 or 2009 if the file or folder is not found
                if result != 0 && result != 2005 && result != 2009 {
                    return Err(Error::new(ErrorKind::Unexpected, format!("{resp:?}")));
                }

                Ok(())
            }
            _ => Err(parse_error(
                ErrorContext::new(if path.ends_with('/') {
                    ServiceOperation("DeleteFolder")
                } else {
                    ServiceOperation("DeleteFile")
                }),
                resp,
            )),
        }
    }
}
