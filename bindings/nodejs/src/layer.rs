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

use std::time::Duration;

use napi::bindgen_prelude::External;
use napi::bindgen_prelude::Result;

pub trait NodeLayer: Send + Sync {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator;
}

/// A public layer wrapper
#[napi]
pub struct Layer {
    pub(crate) inner: Box<dyn NodeLayer>,
}

impl NodeLayer for opendal::layers::RetryLayer {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator {
        op.layer(self.clone())
    }
}

impl NodeLayer for opendal::layers::CapabilityOverrideLayer {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator {
        op.layer(self.clone())
    }
}

/// Capability override layer
///
/// Override the full capability exposed by an operator.
#[napi]
pub struct CapabilityOverrideLayer {
    overrides: String,
}

#[napi]
impl CapabilityOverrideLayer {
    /// Create a new CapabilityOverrideLayer from capability override entries.
    #[napi(constructor)]
    pub fn new(overrides: String) -> Self {
        Self { overrides }
    }

    #[napi]
    pub fn build(&self) -> Result<External<Layer>> {
        let l = opendal::layers::CapabilityOverrideLayer::from_overrides(&self.overrides)
            .map_err(|err| napi::Error::from_reason(format!("{err}")))?;

        Ok(External::new(Layer { inner: Box::new(l) }))
    }
}

/// Retry layer
///
/// Add retry for temporary failed operations.
///
/// # Notes
///
/// This layer will retry failed operations when [`Error::is_temporary`]
/// returns true.
/// If the operation still failed, this layer will set error to
/// `Persistent` which means error has been retried.
///
/// `write` and `blocking_write` don't support retry so far,
/// visit [this issue](https://github.com/apache/opendal/issues/1223) for more details.
///
/// # Examples
///
/// ```javascript
/// const op = new Operator("fs", { root: "/tmp" })
///
/// const retry = new RetryLayer();
/// retry.max_times = 3;
/// retry.jitter = true;
///
/// op.layer(retry.build());
/// ```
#[derive(Default)]
#[napi]
pub struct RetryLayer {
    jitter: bool,
    max_times: Option<u32>,
    factor: Option<f64>,
    max_delay: Option<f64>,
    min_delay: Option<f64>,
}

#[napi]
impl RetryLayer {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set jitter of current backoff.
    ///
    /// If jitter is enabled, ExponentialBackoff will add a random jitter in `[0, min_delay)`
    /// to current delay.
    #[napi(setter)]
    pub fn jitter(&mut self, v: bool) {
        self.jitter = v;
    }

    /// Set max_times of current backoff.
    ///
    /// Backoff will return `None` if max times are reached.
    #[napi(setter)]
    pub fn max_times(&mut self, v: u32) {
        self.max_times = Some(v);
    }

    /// Set factor of current backoff.
    ///
    /// # Panics
    ///
    /// This function will panic if the input factor is smaller than `1.0`.
    #[napi(setter)]
    pub fn factor(&mut self, v: f64) {
        self.factor = Some(v);
    }

    /// Set max_delay of current backoff.
    ///
    /// Delay will not increase if the current delay is larger than max_delay.
    ///
    /// # Notes
    ///
    /// - The unit of max_delay is millisecond.
    #[napi(setter)]
    pub fn max_delay(&mut self, v: f64) {
        self.max_delay = Some(v);
    }

    /// Set min_delay of current backoff.
    ///
    /// # Notes
    ///
    /// - The unit of min_delay is millisecond.
    #[napi(setter)]
    pub fn min_delay(&mut self, v: f64) {
        self.min_delay = Some(v);
    }

    #[napi]
    pub fn build(&self) -> External<Layer> {
        let mut l = opendal::layers::RetryLayer::default();
        if self.jitter {
            l = l.with_jitter();
        }
        if let Some(max_times) = self.max_times {
            l = l.with_max_times(max_times as usize);
        }
        if let Some(factor) = self.factor {
            l = l.with_factor(factor as f32);
        }
        if let Some(max_delay) = self.max_delay {
            l = l.with_max_delay(Duration::from_millis(max_delay as u64));
        }
        if let Some(min_delay) = self.min_delay {
            l = l.with_min_delay(Duration::from_millis(min_delay as u64));
        }

        External::new(Layer { inner: Box::new(l) })
    }
}

impl NodeLayer for opendal::layers::ConcurrentLimitLayer {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator {
        op.layer(self.clone())
    }
}

/// Concurrent limit layer
///
/// Add concurrent request limit.
///
/// # Notes
///
/// Users can control how many concurrent connections could be established
/// between OpenDAL and underlying storage services.
///
/// All operators wrapped by this layer will share a common semaphore.
///
/// # Examples
///
/// ```javascript
/// const op = new Operator("fs", { root: "/tmp" })
///
/// // Create a concurrent limit layer with 1024 permits
/// const limit = new ConcurrentLimitLayer(1024);
/// op.layer(limit.build());
/// ```
///
/// With HTTP concurrent limit:
///
/// ```javascript
/// const limit = new ConcurrentLimitLayer(1024);
/// limit.httpPermits = 512;
/// op.layer(limit.build());
/// ```
#[napi]
pub struct ConcurrentLimitLayer {
    permits: i64,
    http_permits: Option<i64>,
}

#[napi]
impl ConcurrentLimitLayer {
    /// Create a new ConcurrentLimitLayer with specified permits.
    ///
    /// This permits will be applied to all operations.
    ///
    /// # Arguments
    ///
    /// * `permits` - The maximum number of concurrent operations allowed.
    #[napi(constructor)]
    pub fn new(permits: i64) -> Self {
        Self {
            permits,
            http_permits: None,
        }
    }

    /// Set a concurrent limit for HTTP requests.
    ///
    /// This will limit the number of concurrent HTTP requests made by the operator.
    ///
    /// # Arguments
    ///
    /// * `v` - The maximum number of concurrent HTTP requests allowed.
    #[napi(setter)]
    pub fn http_permits(&mut self, v: i64) {
        self.http_permits = Some(v);
    }

    /// Build the layer.
    ///
    /// Returns an `External<Layer>` that can be used with `Operator.layer()`.
    #[napi]
    pub fn build(&self) -> External<Layer> {
        let permits = self.permits;
        let mut l = opendal::layers::ConcurrentLimitLayer::new(permits as usize);

        if let Some(http_permits) = self.http_permits {
            l = l.with_http_concurrent_limit(http_permits as usize);
        }

        External::new(Layer { inner: Box::new(l) })
    }
}

impl NodeLayer for opendal::layers::TimeoutLayer {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator {
        op.layer(self.clone())
    }
}

/// Timeout layer
///
/// Add timeout for every operation to avoid slow or unexpected hang operations.
///
/// # Notes
///
/// `TimeoutLayer` treats all operations in two kinds:
///
/// - Non IO Operation like `stat`, `delete` they operate on a single file. We control
///   them by setting `timeout`.
/// - IO Operation like `read`, `Reader::read` and `Writer::write`, they operate on data directly, we
///   control them by setting `io_timeout`.
///
/// # Default
///
/// - timeout: 60 seconds
/// - io_timeout: 10 seconds
///
/// # Examples
///
/// ```javascript
/// const op = new Operator("fs", { root: "/tmp" })
///
/// const timeout = new TimeoutLayer();
/// timeout.timeout = 10000;      // 10 seconds for non-IO ops (in milliseconds)
/// timeout.ioTimeout = 3000;     // 3 seconds for IO ops (in milliseconds)
///
/// op.layer(timeout.build());
/// ```
#[derive(Default)]
#[napi]
pub struct TimeoutLayer {
    timeout: Option<f64>,
    io_timeout: Option<f64>,
}

#[napi]
impl TimeoutLayer {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set timeout for non-IO operations (stat, delete, etc.)
    ///
    /// # Notes
    ///
    /// - The unit is millisecond.
    /// - Default is 60000ms (60 seconds).
    #[napi(setter)]
    pub fn timeout(&mut self, v: f64) {
        self.timeout = Some(v);
    }

    /// Set timeout for IO operations (read, write, etc.)
    ///
    /// # Notes
    ///
    /// - The unit is millisecond.
    /// - Default is 10000ms (10 seconds).
    #[napi(setter)]
    pub fn io_timeout(&mut self, v: f64) {
        self.io_timeout = Some(v);
    }

    #[napi]
    pub fn build(&self) -> External<Layer> {
        let mut l = opendal::layers::TimeoutLayer::default();

        if let Some(timeout) = self.timeout {
            l = l.with_timeout(Duration::from_millis(timeout as u64));
        }
        if let Some(io_timeout) = self.io_timeout {
            l = l.with_io_timeout(Duration::from_millis(io_timeout as u64));
        }

        External::new(Layer { inner: Box::new(l) })
    }
}

impl NodeLayer for opendal::layers::LoggingLayer {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator {
        op.layer(opendal::layers::LoggingLayer::default())
    }
}

/// Logging layer
///
/// Add log for every operation.
///
/// # Logging
///
/// - OpenDAL will log in structural way.
/// - Every operation will start with a `started` log entry.
/// - Every operation will finish with the following status:
///   - `succeeded`: the operation is successful, but might have more to take.
///   - `finished`: the whole operation is finished.
///   - `failed`: the operation returns an unexpected error.
/// - The default log level while expected error happened is `Warn`.
/// - The default log level while unexpected failure happened is `Error`.
///
/// # Examples
///
/// ```javascript
/// const op = new Operator("fs", { root: "/tmp" })
///
/// const logging = new LoggingLayer();
/// op.layer(logging.build());
/// ```
///
/// # Output
///
/// To enable logging output, set the `RUST_LOG` environment variable:
///
/// ```shell
/// RUST_LOG=debug node app.js
/// ```
#[napi]
pub struct LoggingLayer;

impl Default for LoggingLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[napi]
impl LoggingLayer {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self
    }

    #[napi]
    pub fn build(&self) -> External<Layer> {
        let l = opendal::layers::LoggingLayer::default();
        External::new(Layer { inner: Box::new(l) })
    }
}

impl NodeLayer for opendal::layers::ThrottleLayer {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator {
        op.layer(self.clone())
    }
}

/// Throttle layer
///
/// Add a bandwidth rate limiter to the underlying services.
///
/// # Throttle
///
/// There are several algorithms when it come to rate limiting techniques.
/// This throttle layer uses Generic Cell Rate Algorithm (GCRA) provided by Governor.
/// By setting the `bandwidth` and `burst`, we can control the byte flow rate of underlying services.
///
/// # Note
///
/// When setting the ThrottleLayer, always consider the largest possible operation size as the burst size,
/// as **the burst size should be larger than any possible byte length to allow it to pass through**.
///
/// # Examples
///
/// This example limits bandwidth to 10 KiB/s and burst size to 10 MiB.
///
/// ```javascript
/// const op = new Operator("fs", { root: "/tmp" })
///
/// const throttle = new ThrottleLayer(10 * 1024, 10000 * 1024);
/// op.layer(throttle.build());
/// ```
#[napi]
pub struct ThrottleLayer {
    bandwidth: u32,
    burst: u32,
}

#[napi]
impl ThrottleLayer {
    /// Create a new `ThrottleLayer` with given bandwidth and burst.
    ///
    /// # Arguments
    ///
    /// - `bandwidth`: the maximum number of bytes allowed to pass through per second.
    /// - `burst`: the maximum number of bytes allowed to pass through at once.
    ///
    /// # Notes
    ///
    /// Validation (bandwidth and burst must be greater than 0) is handled by the Rust core layer.
    #[napi(constructor)]
    pub fn new(bandwidth: u32, burst: u32) -> Self {
        Self { bandwidth, burst }
    }

    #[napi]
    pub fn build(&self) -> External<Layer> {
        let l = opendal::layers::ThrottleLayer::new(self.bandwidth, self.burst);
        External::new(Layer { inner: Box::new(l) })
    }
}
