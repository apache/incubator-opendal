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

use async_trait::async_trait;
use bytes;
use bytes::Bytes;
use tokio::runtime::Handle;

use crate::raw::oio::ReadExt;
use crate::raw::*;
use crate::*;

/// Add blocking API support for every operations.
///
/// # Blocking API
///
/// - This layer is auto-added to the operator if it's accessor doesn't support blocking APIs.
///
/// Tracking issue: #2678
#[derive(Debug, Clone)]
pub struct BlockingLayer {
    handle: Handle,
}

impl BlockingLayer {
    /// Create a new `BlockingLayer` with the current runtime's handle
    pub fn create() -> Result<Self> {
        Ok(Self {
            handle: Handle::try_current()
                .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to get current handle"))?,
        })
    }
}

impl<A: Accessor> Layer<A> for BlockingLayer {
    type LayeredAccessor = BlockingAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccessor {
        BlockingAccessor {
            inner,
            handle: self.handle.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlockingAccessor<A: Accessor> {
    inner: A,

    handle: Handle,
}

#[async_trait]
impl<A: Accessor> LayeredAccessor for BlockingAccessor<A> {
    type Inner = A;
    type Reader = A::Reader;
    type BlockingReader = BlockingWrapper<A::Reader>;
    type Writer = A::Writer;
    type BlockingWriter = BlockingWrapper<A::Writer>;
    type Appender = A::Appender;
    type Pager = A::Pager;
    type BlockingPager = BlockingWrapper<A::Pager>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn metadata(&self) -> AccessorInfo {
        let mut info = self.inner.info();
        let cap = info.capability_mut();
        cap.blocking = true;
        info
    }

    async fn create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        self.inner.create_dir(path, args).await
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        self.inner.read(path, args).await
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        self.inner.write(path, args).await
    }

    async fn append(&self, path: &str, args: OpAppend) -> Result<(RpAppend, Self::Appender)> {
        self.inner.append(path, args).await
    }

    async fn copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        self.inner.copy(from, to, args).await
    }

    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.inner.rename(from, to, args).await
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(path, args).await
    }

    async fn delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        self.inner.delete(path, args).await
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Pager)> {
        self.inner.list(path, args).await
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        self.inner.presign(path, args).await
    }

    async fn batch(&self, args: OpBatch) -> Result<RpBatch> {
        self.inner.batch(args).await
    }

    fn blocking_create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        self.handle.block_on(self.inner.create_dir(path, args))
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        self.handle.block_on(async {
            let (rp, reader) = self.inner.read(path, args).await?;
            let blocking_reader = Self::BlockingReader::new(self.handle.clone(), reader);

            Ok((rp, blocking_reader))
        })
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        self.handle.block_on(async {
            let (rp, writer) = self.inner.write(path, args).await?;
            let blocking_writer = Self::BlockingWriter::new(self.handle.clone(), writer);
            Ok((rp, blocking_writer))
        })
    }

    fn blocking_copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        self.handle.block_on(self.inner.copy(from, to, args))
    }

    fn blocking_rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.handle.block_on(self.inner.rename(from, to, args))
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.handle.block_on(self.inner.stat(path, args))
    }

    fn blocking_delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        self.handle.block_on(self.inner.delete(path, args))
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingPager)> {
        self.handle.block_on(async {
            let (rp, pager) = self.inner.list(path, args).await?;
            let blocking_pager = Self::BlockingPager::new(self.handle.clone(), pager);
            Ok((rp, blocking_pager))
        })
    }
}

pub struct BlockingWrapper<I> {
    handle: Handle,
    inner: I,
}

impl<I> BlockingWrapper<I> {
    fn new(handle: Handle, inner: I) -> Self {
        Self { handle, inner }
    }
}

impl<I: oio::Read + 'static> oio::BlockingRead for BlockingWrapper<I> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.handle.block_on(self.inner.read(buf))
    }

    fn seek(&mut self, pos: std::io::SeekFrom) -> Result<u64> {
        self.handle.block_on(self.inner.seek(pos))
    }

    fn next(&mut self) -> Option<Result<Bytes>> {
        self.handle.block_on(self.inner.next())
    }
}

impl<I: oio::Write + 'static> oio::BlockingWrite for BlockingWrapper<I> {
    fn write(&mut self, bs: Bytes) -> Result<()> {
        self.handle.block_on(self.inner.write(bs))
    }

    fn close(&mut self) -> Result<()> {
        self.handle.block_on(self.inner.close())
    }
}

impl<I: oio::Page> oio::BlockingPage for BlockingWrapper<I> {
    fn next(&mut self) -> Result<Option<Vec<oio::Entry>>> {
        self.handle.block_on(self.inner.next())
    }
}

#[cfg(test)]
mod tests {
    use once_cell::sync::Lazy;

    use super::*;
    use crate::types::Result;

    static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    });

    fn create_blocking_layer() -> Result<BlockingLayer> {
        let _guard = RUNTIME.enter();
        BlockingLayer::create()
    }

    #[test]
    fn test_blocking_layer_in_blocking_context() {
        // create in a blocking context should fail
        let layer = BlockingLayer::create();
        assert!(layer.is_err());

        // create in an async context and drop in a blocking context
        let layer = create_blocking_layer();
        assert!(layer.is_ok())
    }

    #[test]
    fn test_blocking_layer_in_async_context() {
        // create and drop in an async context
        let _guard = RUNTIME.enter();

        let layer = BlockingLayer::create();
        assert!(layer.is_ok());
    }
}
