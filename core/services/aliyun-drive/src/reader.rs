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

use super::backend::*;
use super::core::parse_error;
use super::core::*;
use bytes::Buf;
use http::Response;
use http::StatusCode;
use opendal_core::raw::*;
use opendal_core::*;

/// Reader returned by this backend.
pub struct AliyunDriveReader {
    backend: AliyunDriveBackend,
    ctx: OperationContext,
    path: String,
}

impl AliyunDriveReader {
    pub(super) fn new(
        backend: AliyunDriveBackend,
        ctx: OperationContext,
        path: &str,
        _: OpRead,
    ) -> Self {
        Self {
            backend,
            ctx,
            path: path.to_string(),
        }
    }
}

impl oio::StreamRead for AliyunDriveReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let res = backend.core.get_by_path(&self.ctx, path).await?;
        let file: AliyunDriveFile =
            serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;
        let resp = backend
            .core
            .download(&self.ctx, &file.file_id, range)
            .await?;

        let status = resp.status();
        let (rp, stream) = match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => (
                RpRead::new(parse_into_metadata(path, resp.headers())?),
                resp.into_body(),
            ),
            _ => {
                let (part, mut body) = resp.into_parts();
                let buf = body.to_buffer().await?;
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("DownloadFile")),
                    Response::from_parts(part, buf),
                ));
            }
        };

        Ok((rp, Box::new(stream) as Box<dyn oio::ReadStreamDyn>))
    }
}
