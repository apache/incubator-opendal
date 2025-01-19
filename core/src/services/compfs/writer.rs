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

use super::core::CompfsCore;
use crate::raw::*;
use crate::*;

#[derive(Debug)]
pub struct CompfsWriter {
    core: Arc<CompfsCore>,
    file: Cursor<File>,
}

impl CompfsWriter {
    pub(super) fn new(core: Arc<CompfsCore>, file: Cursor<File>) -> Self {
        Self { core, file }
    }
}

impl oio::Write for CompfsWriter {
    /// FIXME
    ///
    /// the write_all doesn't work correctly if `bs` is non-contiguous.
    ///
    /// The IoBuf::buf_len() only returns the length of the current buffer.
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let mut file = self.file.clone();

        self.core
            .exec(move || async move {
                buf_try!(@try file.write_all(bs).await);
                Ok(())
            })
            .await?;

        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let f = self.file.clone();

        self.core
            .exec(move || async move { f.get_ref().sync_all().await })
            .await?;

        let f = self.file.clone();
        self.core
            .exec(move || async move { f.into_inner().close().await })
            .await?;

        Ok(Metadata::default())
    }

    async fn abort(&mut self) -> Result<()> {
        Ok(())
    }
}
