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

use std::io::Cursor;
use std::sync::Arc;

use compio::buf::buf_try;
use compio::fs::File;
use compio::io::AsyncWriteExt;

use super::core::{CompfsBuffer, CompfsCore};
use opendal_core::raw::*;
use opendal_core::*;

#[derive(Debug)]
pub struct CompfsWriter {
    core: Arc<CompfsCore>,
    file: Option<Cursor<File>>,
}

impl CompfsWriter {
    pub(super) fn new(core: Arc<CompfsCore>, file: Cursor<File>) -> Self {
        Self {
            core,
            file: Some(file),
        }
    }
}

impl oio::Write for CompfsWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let Some(mut file) = self.file.clone() else {
            return Err(Error::new(ErrorKind::Unexpected, "file has closed"));
        };

        let compfs_buf = CompfsBuffer::from(bs);
        let pos = self
            .core
            .exec(move || async move {
                buf_try!(@try file.write_vectored_all(compfs_buf).await);
                Ok(file.position())
            })
            .await?;
        self.file.as_mut().unwrap().set_position(pos);

        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let Some(f) = self.file.take() else {
            return Err(Error::new(ErrorKind::Unexpected, "file has closed"));
        };

        self.core
            .exec(move || async move {
                f.get_ref().sync_all().await?;
                f.into_inner().close().await
            })
            .await?;

        Ok(MetadataBuilder::unknown().build())
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "cannot abort completion-based operations",
        ))
    }
}
