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

use bb8::PooledConnection;
use bytes::Buf;
use futures::AsyncWrite;
use futures::AsyncWriteExt;

use super::backend::Manager;
use super::err::parse_error;
use crate::raw::*;
use crate::*;

pub struct FtpWriter {
    target_path: String,
    tmp_path: Option<String>,
    ftp_stream: PooledConnection<'static, Manager>,
    data_stream: Option<Box<dyn AsyncWrite + Sync + Send + Unpin + 'static>>,
}

/// # Safety
///
/// We only have `&mut self` for FtpWrite.
unsafe impl Sync for FtpWriter {}

/// # TODO
///
/// Writer is not implemented correctly.
///
/// After we can use data stream, we should return it directly.
impl FtpWriter {
    pub fn new(
        ftp_stream: PooledConnection<'static, Manager>,
        target_path: String,
        tmp_path: Option<String>,
    ) -> Self {
        FtpWriter {
            target_path,
            tmp_path,
            ftp_stream,
            data_stream: None,
        }
    }
}

impl oio::Write for FtpWriter {
    async fn write(&mut self, mut bs: Buffer) -> Result<()> {
        let path = if let Some(tmp_path) = &self.tmp_path {
            tmp_path
        } else {
            &self.target_path
        };

        if self.data_stream.is_none() {
            self.data_stream = Some(Box::new(
                self.ftp_stream
                    .append_with_stream(path)
                    .await
                    .map_err(parse_error)?,
            ));
        }

        while bs.has_remaining() {
            let n = self
                .data_stream
                .as_mut()
                .unwrap()
                .write(bs.chunk())
                .await
                .map_err(|err| {
                    Error::new(ErrorKind::Unexpected, "copy from ftp stream").set_source(err)
                })?;
            bs.advance(n);
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let data_stream = self.data_stream.take();
        if let Some(mut data_stream) = data_stream {
            data_stream.flush().await.map_err(|err| {
                Error::new(ErrorKind::Unexpected, "flush data stream failed").set_source(err)
            })?;

            self.ftp_stream
                .finalize_put_stream(data_stream)
                .await
                .map_err(parse_error)?;

            if let Some(tmp_path) = &self.tmp_path {
                self.ftp_stream
                    .rename(tmp_path, &self.target_path)
                    .await
                    .map_err(parse_error)?;
            }
        }

        Ok(Metadata::default())
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "FtpWriter doesn't support abort",
        ))
    }
}
