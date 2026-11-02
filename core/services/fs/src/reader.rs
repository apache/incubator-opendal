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

use super::core::*;
use super::lister::FsLister;
use super::writer::FsWriter;
use super::writer::FsWriters;
use opendal_core::raw::*;
use opendal_core::*;
use std::fs::File;
use std::sync::Arc;

/// Reader returned by this backend.
pub struct FsReader {
    core: Arc<FsCore>,
    path: String,
}

impl FsReader {
    pub(super) fn new(core: Arc<FsCore>, path: &str) -> Self {
        Self {
            core,
            path: path.to_string(),
        }
    }
}

pub struct FsReaderHandle {
    core: Arc<FsCore>,
    file: Arc<File>,
}

impl FsReaderHandle {
    pub(super) fn new(core: Arc<FsCore>, file: File) -> Self {
        Self {
            core,
            file: file.into(),
        }
    }
}

impl oio::PositionRead for FsReader {
    type Handle = FsReaderHandle;

    async fn open(&self) -> Result<Self::Handle> {
        let file = self.core.fs_open(&self.path).await?;
        Ok(FsReaderHandle::new(self.core.clone(), file))
    }

    async fn read_at(handle: &Self::Handle, offset: u64, size: usize) -> Result<Buffer> {
        if size == 0 {
            return Ok(Buffer::new());
        }

        let mut bs = handle.core.buf_pool.get();
        bs.resize(size, 0);

        let f = handle.file.clone();
        let (n, mut bs) = tokio::task::spawn_blocking(move || {
            let n = super::backend::read_at(&f, &mut bs, offset)?;
            Ok::<_, Error>((n, bs))
        })
        .await
        .map_err(new_task_join_error)??;

        let frozen = bs.split_to(n).freeze();
        handle.core.buf_pool.put(bs);

        Ok(Buffer::from(frozen))
    }
}

pub struct FsLazyWriter {
    core: Arc<FsCore>,
    executor: Executor,
    path: String,
    op: OpWrite,
    inner: Option<FsWriters>,
}

impl FsLazyWriter {
    pub(super) fn new(core: Arc<FsCore>, executor: Executor, path: &str, op: OpWrite) -> Self {
        Self {
            core,
            executor,
            path: path.to_string(),
            op,
            inner: None,
        }
    }

    async fn inner(&mut self) -> Result<&mut FsWriters> {
        if self.inner.is_none() {
            let is_append = self.op.append();
            let concurrent = self.op.concurrent();
            let writer = FsWriter::create(self.core.clone(), &self.path, self.op.clone()).await?;
            let writer = if is_append {
                FsWriters::One(writer)
            } else {
                FsWriters::Two(oio::PositionWriter::new(
                    self.executor.clone(),
                    writer,
                    concurrent,
                ))
            };

            self.inner = Some(writer);
        }

        Ok(self.inner.as_mut().expect("writer must be initialized"))
    }
}

impl oio::Write for FsLazyWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner().await?.write(bs).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner().await?.close().await
    }

    async fn abort(&mut self) -> Result<()> {
        match self.inner.as_mut() {
            Some(inner) => inner.abort().await,
            None => Ok(()),
        }
    }
}

pub struct FsLazyLister {
    core: Arc<FsCore>,
    path: String,
    inner: Option<Option<FsLister<tokio::fs::ReadDir>>>,
}

impl FsLazyLister {
    pub(super) fn new(core: Arc<FsCore>, path: &str) -> Self {
        Self {
            core,
            path: path.to_string(),
            inner: None,
        }
    }
}

impl oio::List for FsLazyLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        if self.inner.is_none() {
            self.inner = Some(match self.core.fs_list(&self.path).await? {
                Some(rd) => Some(FsLister::new(&self.core.root, &self.path, rd)),
                None => None,
            });
        }

        match self.inner.as_mut().expect("lister must be initialized") {
            Some(lister) => lister.next().await,
            None => Ok(None),
        }
    }
}
