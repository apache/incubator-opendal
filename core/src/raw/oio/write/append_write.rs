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

use std::future::Future;

use crate::raw::*;
use crate::*;

/// AppendWrite is used to implement [`oio::Write`] based on append
/// object. By implementing AppendWrite, services don't need to
/// care about the details of buffering and uploading parts.
///
/// The layout after adopting [`AppendWrite`]:
///
/// - Services impl `AppendWrite`
/// - `AppendWriter` impl `Write`
/// - Expose `AppendWriter` as `Accessor::Writer`
///
/// ## Requirements
///
/// Services that implement `AppendWrite` must fulfill the following requirements:
///
/// - Must be a http service that could accept `AsyncBody`.
/// - Provide a way to get the current offset of the append object.
pub trait AppendWrite: Send + Sync + Unpin + 'static {
    /// Get the current offset of the append object.
    ///
    /// Returns `0` if the object is not exist.
    fn offset(&self) -> impl Future<Output = Result<u64>> + MaybeSend;

    /// Append the data to the end of this object.
    fn append(
        &self,
        offset: u64,
        size: u64,
        body: Buffer,
    ) -> impl Future<Output = Result<()>> + MaybeSend;
}

/// AppendWriter will implements [`oio::Write`] based on append object.
///
/// ## TODO
///
/// - Allow users to switch to un-buffered mode if users write 16MiB every time.
pub struct AppendWriter<W: AppendWrite> {
    inner: W,

    offset: Option<u64>,
}

/// # Safety
///
/// wasm32 is a special target that we only have one event-loop for this state.
impl<W: AppendWrite> AppendWriter<W> {
    /// Create a new AppendWriter.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            offset: None,
        }
    }
}

impl<W> oio::Write for AppendWriter<W>
where
    W: AppendWrite,
{
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let offset = match self.offset {
            Some(offset) => offset,
            None => {
                let offset = self.inner.offset().await?;
                self.offset = Some(offset);
                offset
            }
        };

        let size = bs.len();
        self.inner.append(offset, size as u64, bs).await?;
        // Update offset after succeed.
        self.offset = Some(offset + size as u64);
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        Ok(Metadata::default())
    }

    async fn abort(&mut self) -> Result<()> {
        Ok(())
    }
}
