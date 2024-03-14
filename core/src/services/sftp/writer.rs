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

use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;
use bytes::Bytes;
use openssh_sftp_client::file::File;
use openssh_sftp_client::file::TokioCompatFile;
use tokio::io::AsyncWriteExt;

use crate::raw::{format_std_io_error, new_std_io_error, oio};
use crate::*;

pub struct SftpWriter {
    /// TODO: maybe we can use `File` directly?
    file: Pin<Box<TokioCompatFile>>,
}

impl SftpWriter {
    pub fn new(file: File) -> Self {
        SftpWriter {
            file: Box::pin(TokioCompatFile::new(file)),
        }
    }
}

impl oio::Write for SftpWriter {
    async fn write(&mut self, bs: Bytes) -> Result<usize> {
        self.file.write(&bs).await.map_err(new_std_io_error)
    }

    async fn close(&mut self) -> Result<()> {
        self.file.shutdown().await.map_err(new_std_io_error)
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "SftpWriter doesn't support abort",
        ))
    }
}
