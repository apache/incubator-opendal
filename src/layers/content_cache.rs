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

use std::fmt::Debug;
use std::io::ErrorKind;
use std::io::Result;
use std::pin::Pin;
use std::sync::Arc;
use std::task::ready;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::AsyncRead;
use futures::FutureExt;

use super::util::set_accessor_for_object_iterator;
use super::util::set_accessor_for_object_steamer;
use crate::ops::OpCreate;
use crate::ops::OpDelete;
use crate::ops::OpList;
use crate::ops::OpRead;
use crate::ops::OpStat;
use crate::ops::OpWrite;
use crate::Accessor;
use crate::BlockingBytesReader;
use crate::BytesReader;
use crate::Layer;
use crate::ObjectIterator;
use crate::ObjectReader;
use crate::ObjectStreamer;

/// ContentCacheLayer will add content data cache support for OpenDAL.
///
/// # Notes
///
/// This layer only maintains its own states. Users should care about the cache
/// consistency by themselves. For example, in the following situations, users
/// could get out-dated metadata cache:
///
/// - Users have operations on underlying operator directly.
/// - Other nodes have operations on underlying storage directly.
/// - Concurrent read/write/delete on the same path.
///
/// To make sure content cache consistent across the cluster, please make sure
/// all nodes in the cluster use the same cache services like redis or tikv.
///
/// # Examples
///
/// ```
/// use anyhow::Result;
/// use opendal::layers::ContentCacheLayer;
/// use opendal::services::memory;
/// use opendal::Operator;
/// use opendal::Scheme;
///
/// let _ = Operator::from_env(Scheme::Fs)
///     .expect("must init")
///     .layer(ContentCacheLayer::new(
///         memory::Builder::default().build().expect("must init"),
///     ));
/// ```
#[derive(Debug, Clone)]
pub struct ContentCacheLayer {
    cache: Arc<dyn Accessor>,
}

impl ContentCacheLayer {
    /// Create a new metadata cache layer.
    pub fn new(acc: impl Accessor + 'static) -> Self {
        Self {
            cache: Arc::new(acc),
        }
    }
}

impl Layer for ContentCacheLayer {
    fn layer(&self, inner: Arc<dyn Accessor>) -> Arc<dyn Accessor> {
        Arc::new(ContentCacheAccessor {
            cache: self.cache.clone(),
            inner,
        })
    }
}

#[derive(Debug, Clone)]
struct ContentCacheAccessor {
    cache: Arc<dyn Accessor>,
    inner: Arc<dyn Accessor>,
}

#[async_trait]
impl Accessor for ContentCacheAccessor {
    fn inner(&self) -> Option<Arc<dyn Accessor>> {
        Some(self.inner.clone())
    }

    async fn create(&self, path: &str, args: OpCreate) -> Result<()> {
        self.cache.delete(path, OpDelete::new()).await?;
        self.inner.create(path, args).await
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<ObjectReader> {
        Ok(ObjectReader::new(Box::new(WholeCacheReader::new(
            self.inner.clone(),
            self.cache.clone(),
            path,
            args,
        ))))
    }

    async fn write(&self, path: &str, args: OpWrite, r: BytesReader) -> Result<u64> {
        self.cache.delete(path, OpDelete::new()).await?;
        self.inner.write(path, args, r).await
    }

    async fn delete(&self, path: &str, args: OpDelete) -> Result<()> {
        self.cache.delete(path, OpDelete::new()).await?;
        self.inner.delete(path, args).await
    }

    async fn list(&self, path: &str, args: OpList) -> Result<ObjectStreamer> {
        self.inner
            .list(path, args)
            .await
            .map(|s| set_accessor_for_object_steamer(s, self.clone()))
    }

    fn blocking_create(&self, path: &str, args: OpCreate) -> Result<()> {
        self.cache.blocking_delete(path, OpDelete::new())?;
        self.inner.blocking_create(path, args)
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<BlockingBytesReader> {
        match self.cache.blocking_read(path, args.clone()) {
            Ok(r) => Ok(r),
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let meta = self.inner.blocking_stat(path, OpStat::new())?;
                let r = if meta.mode().is_file() {
                    let size = meta.content_length();
                    let reader = self.inner.blocking_read(path, OpRead::new(..))?;
                    self.cache
                        .blocking_write(path, OpWrite::new(size), reader)?;
                    self.cache.blocking_read(path, args)?
                } else {
                    self.inner.blocking_read(path, args)?
                };
                Ok(r)
            }
            Err(err) => Err(err),
        }
    }

    fn blocking_write(&self, path: &str, args: OpWrite, r: BlockingBytesReader) -> Result<u64> {
        self.cache.blocking_delete(path, OpDelete::new())?;
        self.inner.blocking_write(path, args, r)
    }

    fn blocking_delete(&self, path: &str, args: OpDelete) -> Result<()> {
        self.cache.blocking_delete(path, OpDelete::new())?;
        self.inner.blocking_delete(path, args)
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<ObjectIterator> {
        self.inner
            .blocking_list(path, args)
            .map(|s| set_accessor_for_object_iterator(s, self.clone()))
    }
}

struct WholeCacheReader {
    inner: Arc<dyn Accessor>,
    cache: Arc<dyn Accessor>,
    state: WholeCacheState,

    path: String,
    args: OpRead,
}

impl WholeCacheReader {
    fn new(inner: Arc<dyn Accessor>, cache: Arc<dyn Accessor>, path: &str, args: OpRead) -> Self {
        Self {
            inner,
            cache,
            state: WholeCacheState::Idle,
            path: path.to_string(),
            args,
        }
    }
}

enum WholeCacheState {
    Idle,
    Sending(BoxFuture<'static, Result<ObjectReader>>),
    Reading(ObjectReader),
}

impl AsyncRead for WholeCacheReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        match &mut self.state {
            WholeCacheState::Idle => {
                let cache = self.cache.clone();
                let inner = self.inner.clone();
                let path = self.path.clone();
                let args = self.args.clone();

                let fut = async move {
                    match cache.read(&path, args.clone()).await {
                        Ok(r) => Ok(r),
                        Err(err) if err.kind() == ErrorKind::NotFound => {
                            let r = inner.read(&path, OpRead::new(..)).await?;

                            let length = r.content_length();
                            let size = if let Some(size) = length {
                                size
                            } else {
                                let meta = inner.stat(&path, OpStat::new()).await?;
                                meta.content_length()
                            };
                            cache
                                .write(&path, OpWrite::new(size), r.into_reader())
                                .await?;
                            cache.read(&path, args).await
                        }
                        Err(err) => Err(err),
                    }
                };

                self.state = WholeCacheState::Sending(Box::pin(fut));
                self.poll_read(cx, buf)
            }
            WholeCacheState::Sending(fut) => {
                let r = ready!(fut.poll_unpin(cx))?;
                self.state = WholeCacheState::Reading(r);
                self.poll_read(cx, buf)
            }
            WholeCacheState::Reading(r) => Pin::new(r).poll_read(cx, buf),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::memory;
    use crate::Operator;

    #[tokio::test]
    async fn test_content_cache() -> anyhow::Result<()> {
        let op = Operator::new(memory::Builder::default().build()?);

        let cache_layer = ContentCacheLayer::new(memory::Builder::default().build()?);
        let cached_op = op.clone().layer(cache_layer);

        // Write a new object into op.
        op.object("test_exist")
            .write("Hello, World!".as_bytes())
            .await?;

        // Read from cached op.
        let data = cached_op.object("test_exist").read().await?;
        assert_eq!(data.len(), 13);

        // Write into cache op.
        cached_op
            .object("test_exist")
            .write("Hello, Xuanwo!".as_bytes())
            .await?;
        // op and cached op should have same data.
        let data = op.object("test_exist").read().await?;
        assert_eq!(data.len(), 14);
        let data = cached_op.object("test_exist").read().await?;
        assert_eq!(data.len(), 14);

        // Read not exist object.
        let data = cached_op.object("test_not_exist").read().await;
        assert_eq!(data.unwrap_err().kind(), ErrorKind::NotFound);

        Ok(())
    }
}
