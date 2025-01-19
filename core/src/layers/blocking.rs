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

use std::sync::Arc;

use tokio::runtime::Handle;

use crate::raw::*;
use crate::*;

/// Add blocking API support for non-blocking services.
///
/// # Notes
///
/// - Please only enable this layer when the underlying service does not support blocking.
///
/// # Examples
///
/// ## In async context
///
/// BlockingLayer will use current async context's runtime to handle the async calls.
///
/// ```rust,no_run
/// # use opendal::layers::BlockingLayer;
/// # use opendal::services;
/// # use opendal::BlockingOperator;
/// # use opendal::Operator;
/// # use opendal::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create fs backend builder.
///     let mut builder = services::S3::default().bucket("test").region("us-east-1");
///
///     // Build an `BlockingOperator` with blocking layer to start operating the storage.
///     let _: BlockingOperator = Operator::new(builder)?
///         .layer(BlockingLayer::create()?)
///         .finish()
///         .blocking();
///
///     Ok(())
/// }
/// ```
///
/// ## In async context with blocking functions
///
/// If `BlockingLayer` is called in blocking function, please fetch a [`tokio::runtime::EnterGuard`]
/// first. You can use [`Handle::try_current`] first to get the handle and then call [`Handle::enter`].
/// This often happens in the case that async function calls blocking function.
///
/// ```rust,no_run
/// # use opendal::layers::BlockingLayer;
/// # use opendal::services;
/// # use opendal::BlockingOperator;
/// # use opendal::Operator;
/// # use opendal::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let _ = blocking_fn()?;
///     Ok(())
/// }
///
/// fn blocking_fn() -> Result<BlockingOperator> {
///     // Create fs backend builder.
///     let mut builder = services::S3::default().bucket("test").region("us-east-1");
///
///     let handle = tokio::runtime::Handle::try_current().unwrap();
///     let _guard = handle.enter();
///     // Build an `BlockingOperator` with blocking layer to start operating the storage.
///     let op: BlockingOperator = Operator::new(builder)?
///         .layer(BlockingLayer::create()?)
///         .finish()
///         .blocking();
///     Ok(op)
/// }
/// ```
///
/// ## In blocking context
///
/// In a pure blocking context, we can create a runtime and use it to create the `BlockingLayer`.
///
/// > The following code uses a global statically created runtime as an example, please manage the
/// > runtime on demand.
///
/// ```rust,no_run
/// # use once_cell::sync::Lazy;
/// # use opendal::layers::BlockingLayer;
/// # use opendal::services;
/// # use opendal::BlockingOperator;
/// # use opendal::Operator;
/// # use opendal::Result;
///
/// static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
///     tokio::runtime::Builder::new_multi_thread()
///         .enable_all()
///         .build()
///         .unwrap()
/// });
///
/// fn main() -> Result<()> {
///     // Create fs backend builder.
///     let mut builder = services::S3::default().bucket("test").region("us-east-1");
///
///     // Fetch the `EnterGuard` from global runtime.
///     let _guard = RUNTIME.enter();
///     // Build an `BlockingOperator` with blocking layer to start operating the storage.
///     let _: BlockingOperator = Operator::new(builder)?
///         .layer(BlockingLayer::create()?)
///         .finish()
///         .blocking();
///
///     Ok(())
/// }
/// ```
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

impl<A: Access> Layer<A> for BlockingLayer {
    type LayeredAccess = BlockingAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        BlockingAccessor {
            inner,
            handle: self.handle.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlockingAccessor<A: Access> {
    inner: A,

    handle: Handle,
}

impl<A: Access> LayeredAccess for BlockingAccessor<A> {
    type Inner = A;
    type Reader = A::Reader;
    type BlockingReader = BlockingWrapper<A::Reader>;
    type Writer = A::Writer;
    type BlockingWriter = BlockingWrapper<A::Writer>;
    type Lister = A::Lister;
    type BlockingLister = BlockingWrapper<A::Lister>;
    type Deleter = A::Deleter;
    type BlockingDeleter = BlockingWrapper<A::Deleter>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn info(&self) -> Arc<AccessorInfo> {
        let mut meta = self.inner.info().as_ref().clone();
        meta.full_capability_mut().blocking = true;
        meta.into()
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

    async fn copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        self.inner.copy(from, to, args).await
    }

    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.inner.rename(from, to, args).await
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(path, args).await
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        self.inner.delete().await
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        self.inner.list(path, args).await
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        self.inner.presign(path, args).await
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

    fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
        self.handle.block_on(async {
            let (rp, writer) = self.inner.delete().await?;
            let blocking_deleter = Self::BlockingDeleter::new(self.handle.clone(), writer);
            Ok((rp, blocking_deleter))
        })
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        self.handle.block_on(async {
            let (rp, lister) = self.inner.list(path, args).await?;
            let blocking_lister = Self::BlockingLister::new(self.handle.clone(), lister);
            Ok((rp, blocking_lister))
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
    fn read(&mut self) -> Result<Buffer> {
        self.handle.block_on(self.inner.read())
    }
}

impl<I: oio::Write + 'static> oio::BlockingWrite for BlockingWrapper<I> {
    fn write(&mut self, bs: Buffer) -> Result<()> {
        self.handle.block_on(self.inner.write(bs))
    }

    fn close(&mut self) -> Result<Metadata> {
        self.handle.block_on(self.inner.close())
    }
}

impl<I: oio::List> oio::BlockingList for BlockingWrapper<I> {
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.handle.block_on(self.inner.next())
    }
}

impl<I: oio::Delete + 'static> oio::BlockingDelete for BlockingWrapper<I> {
    fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args)
    }

    fn flush(&mut self) -> Result<usize> {
        self.handle.block_on(self.inner.flush())
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
