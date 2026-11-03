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
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use opendal_core::raw::*;
use opendal_core::*;

/// Builder for TailCutLayer.
///
/// Use this to configure the layer, then call `build()` to create a layer
/// that can be cloned and shared across multiple operators.
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// #
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_tail_cut::TailCutLayer;
/// #
/// # fn main() -> Result<()> {
/// let layer = TailCutLayer::builder()
///     .percentile(95)
///     .window(Duration::from_secs(60))
///     .build();
///
/// let op = Operator::new(services::Memory::default())?
///     .layer(layer);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct TailCutLayerBuilder {
    percentile: u8,
    safety_factor: f64,
    window: Duration,
    min_samples: usize,
    min_deadline: Duration,
    max_deadline: Duration,
}

impl Default for TailCutLayerBuilder {
    fn default() -> Self {
        Self {
            percentile: 95,
            safety_factor: 1.3,
            window: Duration::from_secs(60),
            min_samples: 200,
            min_deadline: Duration::from_millis(500),
            max_deadline: Duration::from_secs(30),
        }
    }
}

impl TailCutLayerBuilder {
    /// Create a new [`TailCutLayerBuilder`] with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the percentile threshold (e.g., 95 for P95, 99 for P99).
    ///
    /// Requests slower than this percentile × safety_factor will be cancelled.
    ///
    /// Default: 95
    ///
    /// # Panics
    ///
    /// Panics if percentile is not between 50 and 99.
    pub fn percentile(mut self, percentile: u8) -> Self {
        assert!(
            (50..=99).contains(&percentile),
            "percentile must be between 50 and 99"
        );
        self.percentile = percentile;
        self
    }

    /// Set the safety factor multiplier.
    ///
    /// The actual deadline is calculated as: P{percentile} × safety_factor.
    /// A higher value reduces false positives but may miss some long tails.
    ///
    /// Default: 1.3 (30% buffer)
    ///
    /// # Panics
    ///
    /// Panics if factor is not between 1.0 and 5.0.
    pub fn safety_factor(mut self, factor: f64) -> Self {
        assert!(
            (1.0..=5.0).contains(&factor),
            "safety_factor must be between 1.0 and 5.0"
        );
        self.safety_factor = factor;
        self
    }

    /// Set the sliding window duration for statistics collection.
    ///
    /// Longer windows provide more stable statistics but react slower to changes.
    /// Shorter windows adapt faster but may be more noisy.
    ///
    /// Default: 60 seconds
    ///
    /// # Panics
    ///
    /// Panics if window is greater than 120 seconds.
    pub fn window(mut self, window: Duration) -> Self {
        assert!(
            window <= Duration::from_secs(120),
            "window must be <= 120 seconds"
        );
        self.window = window;
        self
    }

    /// Set the minimum number of samples required before enabling adaptive cancellation.
    ///
    /// During cold start (when sample count < min_samples), the layer will not
    /// cancel any requests to avoid false positives.
    ///
    /// Default: 200
    pub fn min_samples(mut self, min_samples: usize) -> Self {
        self.min_samples = min_samples;
        self
    }

    /// Set the minimum deadline (floor).
    ///
    /// Even if calculated deadline is shorter, it will be clamped to this value.
    /// This prevents overly aggressive cancellation on very fast backends.
    ///
    /// Default: 500ms
    pub fn min_deadline(mut self, deadline: Duration) -> Self {
        self.min_deadline = deadline;
        self
    }

    /// Set the maximum deadline (ceiling).
    ///
    /// Even if calculated deadline is longer, it will be clamped to this value.
    /// This acts as a safety fallback timeout.
    ///
    /// Default: 30s
    pub fn max_deadline(mut self, deadline: Duration) -> Self {
        self.max_deadline = deadline;
        self
    }

    /// Build the layer.
    ///
    /// The returned layer can be cloned to share statistics across operators.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::time::Duration;
    /// #
    /// # use opendal_core::services;
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # use opendal_layer_tail_cut::TailCutLayer;
    /// #
    /// # fn main() -> Result<()> {
    /// let layer = TailCutLayer::builder()
    ///     .percentile(95)
    ///     .window(Duration::from_secs(60))
    ///     .build();
    ///
    /// // Share the layer across operators
    /// let op1 = Operator::new(services::Memory::default())?
    ///     .layer(layer.clone());
    ///
    /// let op2 = Operator::new(services::Memory::default())?
    ///     .layer(layer.clone());
    /// // op1 and op2 share the same statistics
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(self) -> TailCutLayer {
        TailCutLayer {
            config: Arc::new(TailCutConfig {
                percentile: self.percentile,
                safety_factor: self.safety_factor,
                window: self.window,
                min_samples: self.min_samples,
                min_deadline: self.min_deadline,
                max_deadline: self.max_deadline,
            }),
            stats: Arc::new(TailCutStats::new()),
        }
    }
}

/// Configuration for TailCutLayer (immutable).
#[derive(Debug)]
struct TailCutConfig {
    percentile: u8,
    safety_factor: f64,
    window: Duration,
    min_samples: usize,
    min_deadline: Duration,
    max_deadline: Duration,
}

/// `TailCutLayer` monitors request latency and cancels requests that run
/// significantly slower than the historical baseline (for example, slower than
/// P95).
///
/// Create this layer with [`TailCutLayer::builder()`]. Clone it to share
/// statistics across multiple operators.
///
/// # Examples
///
/// ```no_run
/// # use std::time::Duration;
/// #
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_tail_cut::TailCutLayer;
/// #
/// # fn main() -> Result<()> {
/// let layer = TailCutLayer::builder()
///     .percentile(95)
///     .safety_factor(1.3)
///     .window(Duration::from_secs(60))
///     .build();
///
/// let op = Operator::new(services::Memory::default())?
///     .layer(layer);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct TailCutLayer {
    config: Arc<TailCutConfig>,
    stats: Arc<TailCutStats>,
}

impl Debug for TailCutLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TailCutLayer")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Default for TailCutLayer {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl TailCutLayer {
    /// Create a new [`TailCutLayerBuilder`] to configure the layer.
    pub fn builder() -> TailCutLayerBuilder {
        TailCutLayerBuilder::default()
    }

    /// Create a new [`TailCutLayer`].
    ///
    /// This is equivalent to `TailCutLayer::builder().build()`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for TailCutLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl TailCutLayer {
    fn layer(&self, inner: Servicer) -> TailCutService {
        TailCutService {
            inner,
            config: self.config.clone(),
            stats: self.stats.clone(),
        }
    }
}

#[doc(hidden)]
/// Service wrapper that applies tail-cut deadlines to operation calls.
pub struct TailCutService {
    inner: Servicer,
    config: Arc<TailCutConfig>,
    stats: Arc<TailCutStats>,
}

impl Debug for TailCutService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TailCutService")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl TailCutService {
    /// Calculate the deadline for a given operation and size.
    fn calculate_deadline(&self, op: Operation, size: Option<u64>) -> Option<Duration> {
        let op_stats = self.stats.stats_for(op);

        if op_stats.total_samples(size, self.config.window) < self.config.min_samples {
            return None;
        }

        let q = self.config.percentile as f64 / 100.0;
        let pctl = op_stats.quantile(size, q, self.config.window)?;

        let deadline = Duration::from_secs_f64(pctl.as_secs_f64() * self.config.safety_factor);
        Some(deadline.clamp(self.config.min_deadline, self.config.max_deadline))
    }

    async fn with_deadline<F, T>(&self, op: Operation, size: Option<u64>, fut: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let start = Instant::now();

        let result = if let Some(deadline) = self.calculate_deadline(op, size) {
            match tokio::time::timeout(deadline, fut).await {
                Ok(res) => res,
                Err(_) => Err(Error::new(ErrorKind::Unexpected, "cancelled by tail cut")
                    .with_operation(op)
                    .with_context("percentile", format!("P{}", self.config.percentile))
                    .with_context("deadline", format!("{:?}", deadline))
                    .set_temporary()),
            }
        } else {
            fut.await
        };

        if result.is_ok() {
            let latency = start.elapsed();
            self.stats.stats_for(op).record(size, latency);
        }

        result
    }
}

impl Service for TailCutService {
    type Reader = TailCutWrapper<oio::Reader>;
    type Writer = TailCutWrapper<oio::Writer>;
    type Lister = TailCutWrapper<oio::Lister>;
    type Deleter = TailCutWrapper<oio::Deleter>;
    type Copier = TailCutWrapper<oio::Copier>;
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
        self.with_deadline(
            Operation::CreateDir,
            None,
            self.inner.create_dir(ctx, path, args),
        )
        .await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        self.inner
            .read(ctx, path, args)
            .map(|r| TailCutWrapper::new(r, None, self.config.clone(), self.stats.clone()))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.inner
            .write(ctx, path, args)
            .map(|w| TailCutWrapper::new(w, None, self.config.clone(), self.stats.clone()))
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        self.inner
            .copy(ctx, from, to, args)
            .map(|c| TailCutWrapper::new(c, None, self.config.clone(), self.stats.clone()))
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.with_deadline(
            Operation::Rename,
            None,
            self.inner.rename(ctx, from, to, args),
        )
        .await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.inner.restore(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.with_deadline(Operation::Stat, None, self.inner.stat(ctx, path, args))
            .await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner
            .delete(ctx)
            .map(|d| TailCutWrapper::new(d, None, self.config.clone(), self.stats.clone()))
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.inner
            .list(ctx, path, args)
            .map(|l| TailCutWrapper::new(l, None, self.config.clone(), self.stats.clone()))
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.with_deadline(
            Operation::Presign,
            None,
            self.inner.presign(ctx, path, args),
        )
        .await
    }
}

#[doc(hidden)]
/// Body wrapper that applies tail-cut deadlines to streaming I/O calls.
pub struct TailCutWrapper<R> {
    inner: R,
    size: Option<u64>,
    config: Arc<TailCutConfig>,
    stats: Arc<TailCutStats>,
}

impl<R> TailCutWrapper<R> {
    fn new(
        inner: R,
        size: Option<u64>,
        config: Arc<TailCutConfig>,
        stats: Arc<TailCutStats>,
    ) -> Self {
        Self {
            inner,
            size,
            config,
            stats,
        }
    }

    fn calculate_deadline(&self, op: Operation) -> Option<Duration> {
        self.calculate_deadline_for(op, self.size)
    }

    fn calculate_deadline_for(&self, op: Operation, size: Option<u64>) -> Option<Duration> {
        let op_stats = self.stats.stats_for(op);

        if op_stats.total_samples(size, self.config.window) < self.config.min_samples {
            return None;
        }

        let q = self.config.percentile as f64 / 100.0;
        let pctl = op_stats.quantile(size, q, self.config.window)?;

        let deadline = Duration::from_secs_f64(pctl.as_secs_f64() * self.config.safety_factor);
        Some(deadline.clamp(self.config.min_deadline, self.config.max_deadline))
    }

    #[inline]
    async fn with_io_deadline<F, T>(
        deadline: Option<Duration>,
        percentile: u8,
        stats: &Arc<TailCutStats>,
        size: Option<u64>,
        op: Operation,
        fut: F,
    ) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let start = Instant::now();

        let result = if let Some(dl) = deadline {
            match tokio::time::timeout(dl, fut).await {
                Ok(res) => res,
                Err(_) => Err(
                    Error::new(ErrorKind::Unexpected, "io cancelled by tail cut")
                        .with_operation(op)
                        .with_context("percentile", format!("P{}", percentile))
                        .with_context("deadline", format!("{:?}", dl))
                        .set_temporary(),
                ),
            }
        } else {
            fut.await
        };

        if result.is_ok() {
            let latency = start.elapsed();
            stats.stats_for(op).record(size, latency);
        }

        result
    }
}

impl<R: oio::ReadStream> oio::ReadStream for TailCutWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        let deadline = self.calculate_deadline_for(Operation::Read, self.size);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Read,
            self.inner.read(),
        )
        .await
    }
}

impl<R: oio::Read> oio::Read for TailCutWrapper<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let size = range.size();
        let deadline = self.calculate_deadline_for(Operation::Read, size);
        let (rp, stream) = Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            size,
            Operation::Read,
            self.inner.open(range),
        )
        .await?;

        Ok((
            rp,
            Box::new(TailCutWrapper::new(
                stream,
                size,
                self.config.clone(),
                self.stats.clone(),
            )) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let size = range.size();
        let deadline = self.calculate_deadline_for(Operation::Read, size);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            size,
            Operation::Read,
            self.inner.read(range),
        )
        .await
    }
}

impl<R: oio::Write> oio::Write for TailCutWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let deadline = self.calculate_deadline(Operation::Write);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Write,
            self.inner.write(bs),
        )
        .await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        let deadline = self.calculate_deadline(Operation::Write);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Write,
            self.inner.copy_from(path, args, range),
        )
        .await
    }

    async fn close(&mut self) -> Result<Metadata> {
        let deadline = self.calculate_deadline(Operation::Write);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Write,
            self.inner.close(),
        )
        .await
    }

    async fn abort(&mut self) -> Result<()> {
        let deadline = self.calculate_deadline(Operation::Write);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Write,
            self.inner.abort(),
        )
        .await
    }
}

impl<R: oio::List> oio::List for TailCutWrapper<R> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        let deadline = self.calculate_deadline(Operation::List);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::List,
            self.inner.next(),
        )
        .await
    }
}

impl<R: oio::Delete> oio::Delete for TailCutWrapper<R> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner.delete(path, args).await
    }

    async fn close(&mut self) -> Result<()> {
        let deadline = self.calculate_deadline(Operation::Delete);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Delete,
            self.inner.close(),
        )
        .await
    }
}

impl<C: oio::Copy> oio::Copy for TailCutWrapper<C> {
    async fn next(&mut self) -> Result<Option<usize>> {
        let deadline = self.calculate_deadline(Operation::Copy);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Copy,
            self.inner.next(),
        )
        .await
    }

    async fn close(&mut self) -> Result<Metadata> {
        let deadline = self.calculate_deadline(Operation::Copy);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Copy,
            self.inner.close(),
        )
        .await
    }

    async fn abort(&mut self) -> Result<()> {
        let deadline = self.calculate_deadline(Operation::Copy);
        Self::with_io_deadline(
            deadline,
            self.config.percentile,
            &self.stats,
            self.size,
            Operation::Copy,
            self.inner.abort(),
        )
        .await
    }
}

/// Statistics engine for tail cut layer.
struct TailCutStats {
    // Statistics for each operation type (7 operations)
    operations: [Arc<OperationStats>; 7],
}

impl TailCutStats {
    fn new() -> Self {
        Self {
            operations: std::array::from_fn(|_| Arc::new(OperationStats::new())),
        }
    }

    fn stats_for(&self, op: Operation) -> &Arc<OperationStats> {
        let idx = match op {
            Operation::Read => 0,
            Operation::Write => 1,
            Operation::Stat => 2,
            Operation::List => 3,
            Operation::Delete => 4,
            Operation::Copy => 5,
            Operation::Rename => 6,
            _ => 2, // fallback to Stat
        };
        &self.operations[idx]
    }
}

/// Statistics for a single operation type.
struct OperationStats {
    buckets: Vec<SizeBucket>,
}

impl OperationStats {
    fn new() -> Self {
        Self {
            buckets: vec![
                SizeBucket::new(0, Some(4 * 1024)),                   // [0, 4KB)
                SizeBucket::new(4 * 1024, Some(64 * 1024)),           // [4KB, 64KB)
                SizeBucket::new(64 * 1024, Some(1024 * 1024)),        // [64KB, 1MB)
                SizeBucket::new(1024 * 1024, Some(16 * 1024 * 1024)), // [1MB, 16MB)
                SizeBucket::new(16 * 1024 * 1024, Some(256 * 1024 * 1024)), // [16MB, 256MB)
                SizeBucket::new(256 * 1024 * 1024, None),             // [256MB, ∞)
            ],
        }
    }

    fn bucket_for(&self, size: Option<u64>) -> &SizeBucket {
        let size = size.unwrap_or(u64::MAX);

        self.buckets
            .iter()
            .find(|b| b.contains(size))
            .unwrap_or(&self.buckets[self.buckets.len() - 1])
    }

    fn record(&self, size: Option<u64>, latency: Duration) {
        self.bucket_for(size).histogram.record(latency);
    }

    fn quantile(&self, size: Option<u64>, q: f64, window: Duration) -> Option<Duration> {
        self.bucket_for(size).histogram.quantile(q, window)
    }

    fn total_samples(&self, size: Option<u64>, window: Duration) -> usize {
        self.bucket_for(size).histogram.total_samples(window)
    }
}

/// Size bucket for categorizing operations by data size.
struct SizeBucket {
    min_size: u64,
    max_size: Option<u64>,
    histogram: WindowedHistogram,
}

impl SizeBucket {
    fn new(min_size: u64, max_size: Option<u64>) -> Self {
        Self {
            min_size,
            max_size,
            histogram: WindowedHistogram::new(),
        }
    }

    fn contains(&self, size: u64) -> bool {
        size >= self.min_size && self.max_size.is_none_or(|max| size < max)
    }
}

const SLICE_DURATION_MS: u64 = 10_000; // 10 seconds per slice
const NUM_SLICES: usize = 12; // 12 slices = 120 seconds total window
const NUM_BUCKETS: usize = 17; // 17 buckets covering 1ms to 64s

/// Windowed histogram using lock-free atomic operations.
struct WindowedHistogram {
    slices: Box<[TimeSlice; NUM_SLICES]>,
    current_idx: AtomicUsize,
    last_rotate: AtomicU64,
}

impl WindowedHistogram {
    fn new() -> Self {
        Self {
            slices: Box::new(std::array::from_fn(|_| TimeSlice::new())),
            current_idx: AtomicUsize::new(0),
            last_rotate: AtomicU64::new(Self::now_ms()),
        }
    }

    fn record(&self, latency: Duration) {
        self.maybe_rotate();

        let bucket_idx = Self::latency_to_bucket(latency);
        let slice_idx = self.current_idx.load(Ordering::Relaxed);

        self.slices[slice_idx].buckets[bucket_idx].fetch_add(1, Ordering::Relaxed);
    }

    fn quantile(&self, q: f64, window: Duration) -> Option<Duration> {
        debug_assert!((0.0..=1.0).contains(&q), "quantile must be in [0, 1]");

        let snapshot = self.snapshot(window);
        let total: u64 = snapshot.iter().sum();

        if total == 0 {
            return None;
        }

        let target = (total as f64 * q).ceil() as u64;
        let mut cumsum = 0u64;

        for (bucket_idx, &count) in snapshot.iter().enumerate() {
            cumsum += count;
            if cumsum >= target {
                return Some(Self::bucket_to_latency(bucket_idx));
            }
        }

        Some(Self::bucket_to_latency(NUM_BUCKETS - 1))
    }

    fn total_samples(&self, window: Duration) -> usize {
        self.snapshot(window).iter().map(|&v| v as usize).sum()
    }

    fn snapshot(&self, window: Duration) -> [u64; NUM_BUCKETS] {
        let mut result = [0u64; NUM_BUCKETS];
        let now_ms = Self::now_ms();
        let window_ms = window.as_millis() as u64;

        for slice in self.slices.iter() {
            let start = slice.start_epoch_ms.load(Ordering::Acquire);

            if start > 0 && now_ms.saturating_sub(start) < window_ms + SLICE_DURATION_MS {
                for (i, bucket) in slice.buckets.iter().enumerate() {
                    result[i] += bucket.load(Ordering::Relaxed);
                }
            }
        }

        result
    }

    fn maybe_rotate(&self) {
        let now = Self::now_ms();
        let last_rotate = self.last_rotate.load(Ordering::Relaxed);

        if now - last_rotate >= SLICE_DURATION_MS
            && self
                .last_rotate
                .compare_exchange(last_rotate, now, Ordering::Release, Ordering::Relaxed)
                .is_ok()
        {
            let old_idx = self.current_idx.load(Ordering::Relaxed);
            let new_idx = (old_idx + 1) % NUM_SLICES;

            let new_slice = &self.slices[new_idx];
            new_slice.start_epoch_ms.store(now, Ordering::Release);
            for bucket in &new_slice.buckets {
                bucket.store(0, Ordering::Relaxed);
            }

            self.current_idx.store(new_idx, Ordering::Release);
        }
    }

    fn latency_to_bucket(latency: Duration) -> usize {
        let ms = latency.as_millis() as u64;

        if ms == 0 {
            return 0;
        }

        let bucket = 64 - ms.leading_zeros();
        (bucket as usize).min(NUM_BUCKETS - 1)
    }

    fn bucket_to_latency(bucket_idx: usize) -> Duration {
        if bucket_idx == 0 {
            Duration::from_millis(1)
        } else if bucket_idx >= NUM_BUCKETS - 1 {
            Duration::from_secs(64)
        } else {
            Duration::from_millis(1u64 << bucket_idx)
        }
    }

    fn now_ms() -> u64 {
        // SAFETY: Unless System clock goes backwards before UNIX_EPOCH, this should never fail.
        u64::try_from(Timestamp::now().into_inner().as_millisecond()).unwrap()
    }
}

/// Time slice in the sliding window.
struct TimeSlice {
    // 17 buckets covering 1ms to 64s (logarithmic scale)
    buckets: [AtomicU64; NUM_BUCKETS],
    start_epoch_ms: AtomicU64,
}

impl TimeSlice {
    fn new() -> Self {
        Self {
            buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            start_epoch_ms: AtomicU64::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_to_bucket() {
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_millis(0)),
            0
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_millis(1)),
            1
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_millis(2)),
            2
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_millis(4)),
            3
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_millis(8)),
            4
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_millis(500)),
            9
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_secs(1)),
            10
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_secs(2)),
            11
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_secs(64)),
            16
        );
        assert_eq!(
            WindowedHistogram::latency_to_bucket(Duration::from_secs(1000)),
            16
        );
    }

    #[test]
    fn test_size_bucket_contains() {
        let bucket = SizeBucket::new(0, Some(4096));
        assert!(bucket.contains(0));
        assert!(bucket.contains(4095));
        assert!(!bucket.contains(4096));

        let bucket = SizeBucket::new(4096, None);
        assert!(!bucket.contains(4095));
        assert!(bucket.contains(4096));
        assert!(bucket.contains(u64::MAX));
    }

    #[tokio::test]
    async fn test_histogram_basic() {
        let hist = WindowedHistogram::new();
        let now = WindowedHistogram::now_ms();
        hist.slices[0].start_epoch_ms.store(now, Ordering::Release);

        hist.record(Duration::from_millis(10));
        hist.record(Duration::from_millis(20));
        hist.record(Duration::from_millis(30));

        let samples = hist.total_samples(Duration::from_secs(60));
        assert_eq!(samples, 3);

        let p50 = hist.quantile(0.5, Duration::from_secs(60));
        assert!(p50.is_some());
    }

    #[tokio::test]
    async fn test_tail_cut_layer_build() {
        let layer = TailCutLayer::builder()
            .percentile(95)
            .safety_factor(1.5)
            .window(Duration::from_secs(60))
            .min_samples(100)
            .min_deadline(Duration::from_millis(200))
            .max_deadline(Duration::from_secs(20))
            .build();

        assert_eq!(layer.config.percentile, 95);
        assert_eq!(layer.config.safety_factor, 1.5);
        assert_eq!(layer.config.window, Duration::from_secs(60));
        assert_eq!(layer.config.min_samples, 100);
        assert_eq!(layer.config.min_deadline, Duration::from_millis(200));
        assert_eq!(layer.config.max_deadline, Duration::from_secs(20));
    }

    #[tokio::test]
    async fn test_layer_clone_shares_stats() {
        let layer = TailCutLayer::new();
        let cloned = layer.clone();

        assert!(Arc::ptr_eq(&layer.stats, &cloned.stats));
    }
}
