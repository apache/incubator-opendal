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

/// OneShotWrite is used to implement [`oio::Write`] based on one shot operation.
/// By implementing OneShotWrite, services don't need to care about the details.
///
/// For example, S3 `PUT Object` and fs `write_all`.
///
/// The layout after adopting [`OneShotWrite`]:
pub trait OneShotWrite: Send + Sync + Unpin + 'static {
    /// write_once write all data at once.
    ///
    /// Implementations should make sure that the data is written correctly at once.
    fn write_once(&self, bs: Buffer) -> impl Future<Output = Result<Metadata>> + MaybeSend;
}

/// OneShotWrite is used to implement [`oio::Write`] based on one shot.
pub struct OneShotWriter<W: OneShotWrite> {
    inner: W,
    buffer: Option<Buffer>,
}

impl<W: OneShotWrite> OneShotWriter<W> {
    /// Create a new one shot writer.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            buffer: None,
        }
    }
}

impl<W: OneShotWrite> oio::Write for OneShotWriter<W> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        match &self.buffer {
            Some(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "OneShotWriter doesn't support multiple write",
            )),
            None => {
                self.buffer = Some(bs);
                Ok(())
            }
        }
    }

    async fn close(&mut self) -> Result<Metadata> {
        match self.buffer.clone() {
            Some(bs) => self.inner.write_once(bs).await,
            None => self.inner.write_once(Buffer::new()).await,
        }
    }

    async fn abort(&mut self) -> Result<()> {
        self.buffer = None;
        Ok(())
    }
}
