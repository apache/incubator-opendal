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

use super::core::parse_error;
use super::core::{DropboxCore, DropboxMetadataResponse, ErrorContext};
use opendal_core::raw::*;
use opendal_core::*;

pub struct DropboxWriter {
    core: Arc<DropboxCore>,
    ctx: OperationContext,
    op: OpWrite,
    path: String,
}

impl DropboxWriter {
    pub fn new(core: Arc<DropboxCore>, ctx: OperationContext, op: OpWrite, path: String) -> Self {
        DropboxWriter {
            core,
            ctx,
            op,
            path,
        }
    }

    pub(crate) fn parse_metadata(decoded_response: DropboxMetadataResponse) -> Result<Metadata> {
        let mut metadata = MetadataBuilder::unknown();

        if let Some(size) = decoded_response.size {
            metadata.set_file(size);
        }

        if let Some(content_hash) = decoded_response.content_hash {
            metadata.etag(&content_hash);
        }

        if let Some(rev) = decoded_response.rev {
            metadata.version(&rev);
        }

        if let Some(server_modified) = decoded_response.server_modified {
            let date_utc = server_modified.parse::<Timestamp>()?;
            metadata.last_modified(date_utc);
        } else {
            let date_utc = decoded_response.client_modified.parse::<Timestamp>()?;
            metadata.last_modified(date_utc);
        }

        Ok(metadata.build())
    }
}

impl oio::OneShotWrite for DropboxWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let resp = self
            .core
            .dropbox_update(&self.ctx, &self.path, Some(bs.len()), &self.op, bs)
            .await?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let bytes = resp.into_body();
                let decoded_response: DropboxMetadataResponse =
                    serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;
                let metadata = DropboxWriter::parse_metadata(decoded_response)?;
                Ok(metadata)
            }
            _ => Err(parse_error(
                ErrorContext::new(ServiceOperation("UploadFile")),
                resp,
            )),
        }
    }
}
