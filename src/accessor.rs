// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;

use async_trait::async_trait;
use futures::AsyncRead;

use crate::error::Result;
use crate::io::AsyncReadSeek;
use crate::io::RandomReader;
use crate::io::SequentialReader;
use crate::object::Metadata;
use crate::ops::OpDelete;
use crate::ops::OpRandomRead;
use crate::ops::OpSequentialRead;
use crate::ops::OpStat;
use crate::ops::OpWrite;
use crate::{BoxedAsyncRead, BoxedAsyncReadSeek};

#[async_trait]
pub trait Accessor: Send + Sync {
    /// Read data from the underlying storage into input writer.
    async fn sequential_read(&self, args: &OpSequentialRead) -> Result<BoxedAsyncRead> {
        let _ = args;
        unimplemented!()
    }
    async fn random_read(&self, args: &OpRandomRead) -> Result<BoxedAsyncReadSeek> {
        let _ = args;
        unimplemented!()
    }
    /// Write data from input reader to the underlying storage.
    async fn write(&self, r: BoxedAsyncRead, args: &OpWrite) -> Result<usize> {
        let (_, _) = (r, args);
        unimplemented!()
    }
    /// Invoke the `stat` operation on the specified path.
    async fn stat(&self, args: &OpStat) -> Result<Metadata> {
        let _ = args;
        unimplemented!()
    }
    /// `Delete` will invoke the `delete` operation.
    ///
    /// ## Behavior
    ///
    /// - `Delete` is an idempotent operation, it's safe to call `Delete` on the same path multiple times.
    /// - `Delete` will return `Ok(())` if the path is deleted successfully or not exist.
    async fn delete(&self, args: &OpDelete) -> Result<()> {
        let _ = args;
        unimplemented!()
    }
}

/// All functions in `Accessor` only requires `&self`, so it's safe to implement
/// `Accessor` for `Arc<dyn Accessor>`.
#[async_trait]
impl<T: Accessor> Accessor for Arc<T> {
    async fn sequential_read(&self, args: &OpSequentialRead) -> Result<BoxedAsyncRead> {
        self.as_ref().sequential_read(args).await
    }
    async fn random_read(&self, args: &OpRandomRead) -> Result<BoxedAsyncReadSeek> {
        self.as_ref().random_read(args).await
    }
    async fn write(&self, r: BoxedAsyncRead, args: &OpWrite) -> Result<usize> {
        self.as_ref().write(r, args).await
    }
    async fn stat(&self, args: &OpStat) -> Result<Metadata> {
        self.as_ref().stat(args).await
    }
    async fn delete(&self, args: &OpDelete) -> Result<()> {
        self.as_ref().delete(args).await
    }
}
