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

#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]
use std::num::NonZeroU32;
use std::sync::Arc;

use governor::Quota;
use governor::RateLimiter;
use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use governor::state::InMemoryState;
use governor::state::NotKeyed;
use opendal_core::raw::*;
use opendal_core::*;

/// `ThrottleLayer` limits bandwidth for storage services.
///
/// # Throttle
///
/// This layer uses the Generic Cell Rate Algorithm (GCRA) from
/// [Governor](https://docs.rs/governor/latest/governor/index.html).
/// Set `bandwidth` and `burst` to control the service's byte-flow rate.
/// Reads with a bounded raw range acquire quota for the requested size before
/// starting range I/O. Reads with an unbounded raw range are not throttled.
/// Writes acquire quota for each buffer before sending it to the service.
///
/// # Note
///
/// Set the burst size to at least the largest bounded read range or write buffer.
/// A range or buffer larger than the burst fails with [`ErrorKind::RateLimited`].
///
/// Read more about [Quota](https://docs.rs/governor/latest/governor/struct.Quota.html#examples)
///
/// # Examples
///
/// This example limits bandwidth to 10 KiB/s and burst size to 10 MiB.
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_throttle::ThrottleLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())
///     .expect("must init")
///     .layer(ThrottleLayer::new(10 * 1024, 10000 * 1024));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct ThrottleLayer {
    rate_limiter: SharedRateLimiter,
}

impl ThrottleLayer {
    /// Create a new `ThrottleLayer` with given bandwidth and burst.
    ///
    /// - bandwidth: the maximum number of bytes allowed to pass through per second.
    /// - burst: the maximum number of bytes allowed to pass through at once.
    pub fn new(bandwidth: u32, burst: u32) -> Self {
        assert!(bandwidth > 0);
        assert!(burst > 0);
        Self {
            rate_limiter: Arc::new(RateLimiter::direct(
                Quota::per_second(NonZeroU32::new(bandwidth).unwrap())
                    .allow_burst(NonZeroU32::new(burst).unwrap()),
            )),
        }
    }
}

impl Layer for ThrottleLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl ThrottleLayer {
    fn layer(&self, inner: Servicer) -> ThrottleAccessor {
        ThrottleAccessor {
            inner,
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}

/// Share an atomic RateLimiter instance across all threads in one operator.
/// If want to add more observability in the future, replace the default NoOpMiddleware with other middleware types.
/// Read more about [Middleware](https://docs.rs/governor/latest/governor/middleware/index.html)
type SharedRateLimiter = Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>;

async fn wait_for_quota(limiter: &SharedRateLimiter, len: u64) -> Result<()> {
    let len = u32::try_from(len).map_err(|_| {
        Error::new(
            ErrorKind::RateLimited,
            "request size exceeds throttle quota capacity",
        )
    })?;
    let Some(len) = NonZeroU32::new(len) else {
        return Ok(());
    };

    limiter.until_n_ready(len).await.map_err(|_| {
        Error::new(
            ErrorKind::RateLimited,
            "burst size is smaller than the request size",
        )
    })
}

#[doc(hidden)]
#[derive(Debug)]
pub struct ThrottleAccessor {
    inner: Servicer,
    rate_limiter: SharedRateLimiter,
}

impl Service for ThrottleAccessor {
    type Reader = ThrottleWrapper<oio::Reader>;
    type Writer = ThrottleWrapper<oio::Writer>;
    type Lister = oio::Lister;
    type Deleter = oio::Deleter;
    type Copier = oio::Copier;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.inner.info()
    }

    fn capability(&self) -> Capability {
        self.inner.capability()
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.inner.compose(ctx, to, args)
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.inner.create_dir(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(ctx, path, args).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let limiter = self.rate_limiter.clone();

        self.inner
            .read(ctx, path, args)
            .map(|r| ThrottleWrapper::new(r, limiter))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let limiter = self.rate_limiter.clone();

        self.inner
            .write(ctx, path, args)
            .map(|w| ThrottleWrapper::new(w, limiter))
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        self.inner.copy(ctx, from, to, args)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner.delete(ctx)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.inner.rename(ctx, from, to, args).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.inner.restore(ctx, path, args).await
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.inner.list(ctx, path, args)
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.inner.presign(ctx, path, args).await
    }
}

#[doc(hidden)]
pub struct ThrottleWrapper<R> {
    inner: R,
    limiter: SharedRateLimiter,
}

impl<R> ThrottleWrapper<R> {
    fn new(inner: R, rate_limiter: SharedRateLimiter) -> Self {
        Self {
            inner,
            limiter: rate_limiter,
        }
    }
}

impl<R: oio::ReadStream> oio::ReadStream for ThrottleWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner.read().await
    }
}

impl<R: oio::Read> oio::Read for ThrottleWrapper<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        if let Some(size) = range.size() {
            wait_for_quota(&self.limiter, size).await?;
        }
        let (rp, stream) = self.inner.open(range).await?;
        Ok((
            rp,
            Box::new(ThrottleWrapper::new(stream, self.limiter.clone()))
                as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        if let Some(size) = range.size() {
            wait_for_quota(&self.limiter, size).await?;
        }
        self.inner.read(range).await
    }
}

impl<R: oio::Write> oio::Write for ThrottleWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        wait_for_quota(&self.limiter, bs.len() as u64).await?;
        self.inner.write(bs).await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        self.inner.copy_from(path, args, range).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner.close().await
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner.abort().await
    }
}

#[cfg(test)]
mod tests;
