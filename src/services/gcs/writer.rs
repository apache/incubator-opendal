// Copyright 2022 Datafuse Labs
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use async_trait::async_trait;
use http::StatusCode;

use super::backend::GcsBackend;
use super::error::parse_error;
use crate::ops::OpWrite;
use crate::raw::*;
use crate::*;

pub struct GcsWriter {
    backend: GcsBackend,

    op: OpWrite,
    path: String,
}

impl GcsWriter {
    pub fn new(backend: GcsBackend, op: OpWrite, path: String) -> Self {
        GcsWriter { backend, op, path }
    }
}

#[async_trait]
impl output::Write for GcsWriter {
    async fn write(&mut self, bs: Vec<u8>) -> Result<()> {
        let mut req = self.backend.gcs_insert_object_request(
            &self.path,
            Some(self.op.size()),
            self.op.content_type(),
            AsyncBody::Bytes(bs.into()),
        )?;

        self.backend
            .signer
            .sign(&mut req)
            .map_err(new_request_sign_error)?;

        let resp = self.backend.client.send_async(req).await?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => {
                resp.into_body().consume().await?;
                Ok(())
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn append(&mut self, bs: Vec<u8>) -> Result<()> {
        let _ = bs;

        Err(Error::new(
            ErrorKind::Unsupported,
            "output writer doesn't support append",
        ))
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
