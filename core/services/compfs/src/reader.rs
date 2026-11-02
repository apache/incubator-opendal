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

use super::core::CompfsBuffer;
use super::core::CompfsCore;
use super::lister::CompfsLister;
use super::writer::CompfsWriter;
use compio::buf::IntoInner;
use compio::buf::IoBuf;
use compio::buf::buf_try;
use compio::io::AsyncReadAt;
use opendal_core::raw::*;
use opendal_core::*;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

/// Reader returned by this backend.
pub struct CompfsReader {
    core: Arc<CompfsCore>,
    path: PathBuf,
}

impl CompfsReader {
    pub(super) fn new(core: Arc<CompfsCore>, path: PathBuf) -> Self {
        Self { core, path }
    }
}

pub struct CompfsReaderHandle {
    core: Arc<CompfsCore>,
    file: compio::fs::File,
}

impl CompfsReaderHandle {
    pub(super) fn new(core: Arc<CompfsCore>, file: compio::fs::File) -> Self {
        Self { core, file }
    }
}

impl oio::PositionRead for CompfsReader {
    type Handle = CompfsReaderHandle;

    async fn open(&self) -> Result<Self::Handle> {
        let path = self.path.clone();
        let file = self
            .core
            .exec(move || async move {
                let file = compio::fs::OpenOptions::new()
                    .read(true)
                    .open(&path)
                    .await?;
                Ok(file)
            })
            .await?;

        Ok(CompfsReaderHandle::new(self.core.clone(), file))
    }

    async fn read_at(handle: &Self::Handle, offset: u64, size: usize) -> Result<Buffer> {
        if size == 0 {
            return Ok(Buffer::new());
        }

        let mut bs = handle.core.buf_pool.get();
        bs.reserve(size);

        let file = handle.file.clone();
        let (n, mut bs) = handle
            .core
            .exec(move || async move {
                let (n, bs) = buf_try!(@try file.read_at(bs.slice(..size), offset).await);
                Ok((n, bs.into_inner()))
            })
            .await?;

        let frozen = bs.split_to(n).freeze();
        handle.core.buf_pool.put(bs);

        Ok(CompfsBuffer::from(Buffer::from(frozen)).into())
    }
}

pub struct CompfsLazyWriter {
    core: Arc<CompfsCore>,
    path: PathBuf,
    args: OpWrite,
    inner: Option<CompfsWriter>,
}

impl CompfsLazyWriter {
    pub(super) fn new(core: Arc<CompfsCore>, path: PathBuf, args: OpWrite) -> Self {
        Self {
            core,
            path,
            args,
            inner: None,
        }
    }

    async fn inner(&mut self) -> Result<&mut CompfsWriter> {
        if self.inner.is_none() {
            let path = self.path.clone();
            let append = self.args.append();
            let file = self
                .core
                .exec(move || async move {
                    if let Some(parent) = path.parent() {
                        compio::fs::create_dir_all(parent).await?;
                    }
                    let file = compio::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(!append)
                        .open(path)
                        .await?;
                    let mut file = Cursor::new(file);
                    if append {
                        let len = file.get_ref().metadata().await?.len();
                        file.set_position(len);
                    }
                    Ok(file)
                })
                .await?;

            self.inner = Some(CompfsWriter::new(self.core.clone(), file));
        }

        Ok(self.inner.as_mut().expect("writer must be initialized"))
    }
}

impl oio::Write for CompfsLazyWriter {
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

pub struct CompfsLazyLister {
    core: Arc<CompfsCore>,
    path: PathBuf,
    inner: Option<Option<CompfsLister>>,
}

impl CompfsLazyLister {
    pub(super) fn new(core: Arc<CompfsCore>, path: PathBuf) -> Self {
        Self {
            core,
            path,
            inner: None,
        }
    }
}

impl oio::List for CompfsLazyLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        if self.inner.is_none() {
            let path = self.path.clone();
            self.inner = Some(
                match self
                    .core
                    .exec_blocking({
                        let path = path.clone();
                        move || std::fs::read_dir(path)
                    })
                    .await?
                {
                    Ok(read_dir) => Some(CompfsLister::new(self.core.clone(), &path, read_dir)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => return Err(new_std_io_error(e)),
                },
            );
        }

        match self.inner.as_mut().expect("lister must be initialized") {
            Some(lister) => lister.next().await,
            None => Ok(None),
        }
    }
}
