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

use super::core::HdfsNativeCore;
use super::core::parse_hdfs_error;
use super::lister::HdfsNativeLister;
use super::writer::HdfsNativeWriter;
use opendal_core::raw::*;
use opendal_core::*;
use std::sync::Arc;

/// Reader returned by this backend.
pub struct HdfsNativeReader {
    core: Arc<HdfsNativeCore>,
    path: String,
}

impl HdfsNativeReader {
    pub(super) fn new(core: Arc<HdfsNativeCore>, path: &str) -> Self {
        Self {
            core,
            path: path.to_string(),
        }
    }
}

pub struct HdfsNativeReaderHandle {
    file: hdfs_native::file::FileReader,
}

impl HdfsNativeReaderHandle {
    pub(super) fn new(file: hdfs_native::file::FileReader) -> Self {
        Self { file }
    }
}

impl oio::PositionRead for HdfsNativeReader {
    type Handle = HdfsNativeReaderHandle;

    async fn open(&self) -> Result<Self::Handle> {
        let file = self.core.hdfs_open(&self.path).await?;
        Ok(HdfsNativeReaderHandle::new(file))
    }

    async fn read_at(handle: &Self::Handle, offset: u64, size: usize) -> Result<Buffer> {
        if size == 0 {
            return Ok(Buffer::new());
        }

        let Ok(offset) = usize::try_from(offset) else {
            return Ok(Buffer::new());
        };

        let file_length = handle.file.file_length();
        if offset >= file_length {
            return Ok(Buffer::new());
        }

        let size = size.min(file_length - offset);
        let bytes = handle
            .file
            .read_range(offset, size)
            .await
            .map_err(parse_hdfs_error)?;

        Ok(Buffer::from(bytes))
    }
}

pub struct HdfsNativeLazyWriter {
    core: Arc<HdfsNativeCore>,
    path: String,
    args: OpWrite,
    inner: Option<HdfsNativeWriter>,
}

impl HdfsNativeLazyWriter {
    pub(super) fn new(core: Arc<HdfsNativeCore>, path: &str, args: OpWrite) -> Self {
        Self {
            core,
            path: path.to_string(),
            args,
            inner: None,
        }
    }

    async fn inner(&mut self) -> Result<&mut HdfsNativeWriter> {
        if self.inner.is_none() {
            let (f, initial_size) = self.core.hdfs_write(&self.path, &self.args).await?;
            self.inner = Some(HdfsNativeWriter::new(f, initial_size));
        }

        Ok(self.inner.as_mut().expect("writer must be initialized"))
    }
}

impl oio::Write for HdfsNativeLazyWriter {
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

pub struct HdfsNativeLazyLister {
    core: Arc<HdfsNativeCore>,
    path: String,
    inner: Option<Option<HdfsNativeLister>>,
}

impl HdfsNativeLazyLister {
    pub(super) fn new(core: Arc<HdfsNativeCore>, path: &str) -> Self {
        Self {
            core,
            path: path.to_string(),
            inner: None,
        }
    }
}

impl oio::List for HdfsNativeLazyLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        if self.inner.is_none() {
            self.inner = Some(match self.core.hdfs_list(&self.path).await? {
                Some((p, current_path)) => Some(HdfsNativeLister::new(
                    &self.core.root,
                    &self.core.client,
                    &p,
                    current_path,
                )),
                None => None,
            });
        }

        match self.inner.as_mut().expect("lister must be initialized") {
            Some(lister) => lister.next().await,
            None => Ok(None),
        }
    }
}
