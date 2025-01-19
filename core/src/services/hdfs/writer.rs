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

use std::io::Write;
use std::sync::Arc;

use bytes::Buf;
use futures::AsyncWriteExt;

use crate::raw::*;
use crate::*;

pub struct HdfsWriter<F> {
    target_path: String,
    tmp_path: Option<String>,
    f: Option<F>,
    client: Arc<hdrs::Client>,
    target_path_exists: bool,
}

/// # Safety
///
/// We will only take `&mut Self` reference for HdfsWriter.
unsafe impl<F> Sync for HdfsWriter<F> {}

impl<F> HdfsWriter<F> {
    pub fn new(
        target_path: String,
        tmp_path: Option<String>,
        f: F,
        client: Arc<hdrs::Client>,
        target_path_exists: bool,
    ) -> Self {
        Self {
            target_path,
            tmp_path,
            f: Some(f),
            client,
            target_path_exists,
        }
    }
}

impl oio::Write for HdfsWriter<hdrs::AsyncFile> {
    async fn write(&mut self, mut bs: Buffer) -> Result<()> {
        let f = self.f.as_mut().expect("HdfsWriter must be initialized");

        while bs.has_remaining() {
            let n = f.write(bs.chunk()).await.map_err(new_std_io_error)?;
            bs.advance(n);
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let f = self.f.as_mut().expect("HdfsWriter must be initialized");
        f.close().await.map_err(new_std_io_error)?;

        // TODO: we need to make rename async.
        if let Some(tmp_path) = &self.tmp_path {
            // we must delete the target_path, otherwise the rename_file operation will fail
            if self.target_path_exists {
                self.client
                    .remove_file(&self.target_path)
                    .map_err(new_std_io_error)?;
            }
            self.client
                .rename_file(tmp_path, &self.target_path)
                .map_err(new_std_io_error)?
        }

        Ok(Metadata::default())
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "HdfsWriter doesn't support abort",
        ))
    }
}

impl oio::BlockingWrite for HdfsWriter<hdrs::File> {
    fn write(&mut self, mut bs: Buffer) -> Result<()> {
        let f = self.f.as_mut().expect("HdfsWriter must be initialized");
        while bs.has_remaining() {
            let n = f.write(bs.chunk()).map_err(new_std_io_error)?;
            bs.advance(n);
        }

        Ok(())
    }

    fn close(&mut self) -> Result<Metadata> {
        let f = self.f.as_mut().expect("HdfsWriter must be initialized");
        f.flush().map_err(new_std_io_error)?;

        if let Some(tmp_path) = &self.tmp_path {
            // we must delete the target_path, otherwise the rename_file operation will fail
            if self.target_path_exists {
                self.client
                    .remove_file(&self.target_path)
                    .map_err(new_std_io_error)?;
            }
            self.client
                .rename_file(tmp_path, &self.target_path)
                .map_err(new_std_io_error)?;
        }

        Ok(Metadata::default())
    }
}
