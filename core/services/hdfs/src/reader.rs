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

use super::core::HdfsCore;
use super::writer::HdfsWriter;
use opendal_core::raw::*;
use opendal_core::*;
use std::sync::Arc;

/// Reader returned by this backend.
pub struct HdfsReader {
    core: Arc<HdfsCore>,
    path: String,
}

impl HdfsReader {
    pub(super) fn new(core: Arc<HdfsCore>, path: &str) -> Self {
        Self {
            core,
            path: path.to_string(),
        }
    }
}

pub struct HdfsReaderHandle {
    file: Arc<hdrs::File>,
}

impl HdfsReaderHandle {
    pub(super) fn new(file: hdrs::File) -> Self {
        Self { file: file.into() }
    }
}

impl oio::PositionRead for HdfsReader {
    type Handle = HdfsReaderHandle;

    async fn open(&self) -> Result<Self::Handle> {
        let file = self.core.hdfs_open(&self.path).await?;
        Ok(HdfsReaderHandle::new(file))
    }

    async fn read_at(handle: &Self::Handle, offset: u64, size: usize) -> Result<Buffer> {
        if size == 0 {
            return Ok(Buffer::new());
        }

        let file = handle.file.clone();
        let buf = tokio::task::spawn_blocking(move || {
            let mut buf = vec![0; size];
            let n = file.read_at(&mut buf, offset).map_err(new_std_io_error)?;
            buf.truncate(n);
            Ok::<_, Error>(Buffer::from(buf))
        })
        .await
        .map_err(|e| Error::new(ErrorKind::Unexpected, "tokio task join failed").set_source(e))??;

        Ok(buf)
    }
}

pub struct HdfsLazyWriter {
    core: Arc<HdfsCore>,
    path: String,
    op: OpWrite,
    inner: Option<HdfsWriter<hdrs::AsyncFile>>,
}

impl HdfsLazyWriter {
    pub(super) fn new(core: Arc<HdfsCore>, path: &str, op: OpWrite) -> Self {
        Self {
            core,
            path: path.to_string(),
            op,
            inner: None,
        }
    }

    async fn inner(&mut self) -> Result<&mut HdfsWriter<hdrs::AsyncFile>> {
        if self.inner.is_none() {
            let (target_path, tmp_path, f, target_exists, initial_size) =
                self.core.hdfs_write(&self.path, &self.op).await?;

            self.inner = Some(HdfsWriter::new(
                target_path,
                tmp_path,
                f,
                Arc::clone(&self.core.client),
                target_exists,
                initial_size,
            ));
        }

        Ok(self.inner.as_mut().expect("writer must be initialized"))
    }
}

impl oio::Write for HdfsLazyWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner().await?.write(bs).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner().await?.close().await
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner().await?.abort().await
    }
}
