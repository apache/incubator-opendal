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

//! # OpenDAL Metrics Reference
//!
//! This document describes all metrics exposed by OpenDAL.
//!
//! ## Operation Metrics
//!
//! These metrics track operations at the storage abstraction level.
//!
//! | Metric Name                      | Type      | Description                                                                                | Labels                                          |
//! |----------------------------------|-----------|--------------------------------------------------------------------------------------------|-------------------------------------------------|
//! | operation_bytes                  | Histogram | Current operation size in bytes, represents the size of data being processed               | scheme, namespace, root, operation, path        |
//! | operation_bytes_rate             | Histogram | Histogram of data processing rates in bytes per second within individual operations        | scheme, namespace, root, operation, path        |
//! | operation_entries                | Histogram | Current operation size in entries, represents the entries being processed                  | scheme, namespace, root, operation, path        |
//! | operation_entries_rate           | Histogram | Histogram of entries processing rates in entries per second within individual operations   | scheme, namespace, root, operation, path        |
//! | operation_duration_seconds       | Histogram | Duration of operations in seconds, measured from start to completion                       | scheme, namespace, root, operation, path        |
//! | operation_errors_total           | Counter   | Total number of failed operations                                                          | scheme, namespace, root, operation, path, error |
//! | operation_executing              | Gauge     | Number of operations currently being executed                                              | scheme, namespace, root, operation              |
//! | operation_ttfb_seconds           | Histogram | Time to first byte in seconds for operations                                               | scheme, namespace, root, operation, path        |
//!
//! ## HTTP Metrics
//!
//! These metrics track the underlying HTTP requests made by OpenDAL services that use HTTP.
//!
//! | Metric Name                      | Type      | Description                                                                                | Labels                                          |
//! |----------------------------------|-----------|--------------------------------------------------------------------------------------------|-------------------------------------------------|
//! | http_connection_errors_total     | Counter   | Total number of HTTP requests that failed before receiving a response                      | scheme, namespace, root, operation, error       |
//! | http_status_errors_total         | Counter   | Total number of HTTP requests that received error status codes (non-2xx responses)         | scheme, namespace, root, operation, status      |
//! | http_executing                   | Gauge     | Number of HTTP requests currently in flight from this client                               | scheme, namespace, root                         |
//! | http_request_bytes               | Histogram | Histogram of HTTP request body sizes in bytes                                              | scheme, namespace, root, operation              |
//! | http_request_bytes_rate          | Histogram | Histogram of HTTP request bytes per second rates                                           | scheme, namespace, root, operation              |
//! | http_request_duration_seconds    | Histogram | Histogram of time spent sending HTTP requests, from first byte sent to first byte received | scheme, namespace, root, operation              |
//! | http_response_bytes              | Histogram | Histogram of HTTP response body sizes in bytes                                             | scheme, namespace, root, operation              |
//! | http_response_bytes_rate         | Histogram | Histogram of HTTP response bytes per second rates                                          | scheme, namespace, root, operation              |
//! | http_response_duration_seconds   | Histogram | Histogram of time spent receiving HTTP responses, from first byte to last byte received    | scheme, namespace, root, operation              |
//!
//! ## Label Descriptions
//!
//! | Label     | Description                                                   | Example Values                         |
//! |-----------|---------------------------------------------------------------|----------------------------------------|
//! | scheme    | The storage service scheme                                    | s3, gcs, azblob, fs, memory            |
//! | namespace | The storage service namespace (bucket, container, etc.)       | my-bucket, my-container                |
//! | root      | The root path within the namespace                            | /data, /backup                         |
//! | operation | The operation being performed                                 | read, write, stat, list, delete        |
//! | path      | The path of the object being operated on                      | /path/to/file.txt                      |
//! | error     | The error type or message for error metrics                   | not_found, permission_denied           |
//! | status    | The HTTP status code for HTTP error metrics                   | 404, 403, 500                          |
//!
//! ## Metric Types
//!
//! * **Histogram**: Distribution of values with configurable buckets, includes count, sum and quantiles
//! * **Counter**: Cumulative metric that only increases over time (resets on restart)
//! * **Gauge**: Point-in-time metric that can increase and decrease

use std::fmt::Debug;
use std::fmt::Formatter;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use futures::Stream;
use futures::StreamExt;
use http::StatusCode;
use opendal_core::raw::*;
use opendal_core::*;

const KIB: f64 = 1024.0;
const MIB: f64 = 1024.0 * KIB;
const GIB: f64 = 1024.0 * MIB;

/// Buckets for data size metrics like OperationBytes
/// Covers typical file and object sizes from small files to large objects
pub const DEFAULT_BYTES_BUCKETS: &[f64] = &[
    4.0 * KIB,   // Small files
    64.0 * KIB,  // File system block size
    256.0 * KIB, //
    1.0 * MIB,   // Common size threshold for many systems
    4.0 * MIB,   // Best size for most http based systems
    16.0 * MIB,  //
    64.0 * MIB,  // Widely used threshold for multipart uploads
    256.0 * MIB, //
    1.0 * GIB,   // Considered large for many systems
    5.0 * GIB,   // Maximum size in single upload for many cloud storage services
];

/// Buckets for data transfer rate metrics like OperationBytesRate
///
/// Covers various network speeds from slow connections to high-speed transfers
///
/// Note: this is for single operation rate, not for total bandwidth.
pub const DEFAULT_BYTES_RATE_BUCKETS: &[f64] = &[
    // Low-speed network range (mobile/weak connections)
    8.0 * KIB,   // ~64Kbps - 2G networks
    32.0 * KIB,  // ~256Kbps - 3G networks
    128.0 * KIB, // ~1Mbps - Basic broadband
    // Standard broadband range
    1.0 * MIB,  // ~8Mbps - Entry-level broadband
    8.0 * MIB,  // ~64Mbps - Fast broadband
    32.0 * MIB, // ~256Mbps - Gigabit broadband
    // High-performance network range
    128.0 * MIB, // ~1Gbps - Standard datacenter
    512.0 * MIB, // ~4Gbps - Fast datacenter
    2.0 * GIB,   // ~16Gbps - High-end interconnects
    // Ultra-high-speed range
    8.0 * GIB,  // ~64Gbps - InfiniBand/RDMA
    32.0 * GIB, // ~256Gbps - Top-tier datacenters
];

/// Buckets for batch operation entry counts (OperationEntriesCount)
/// Covers scenarios from single entry operations to large batch operations
pub const DEFAULT_ENTRIES_BUCKETS: &[f64] = &[
    1.0,     // Single item operations
    5.0,     // Very small batches
    10.0,    // Small batches
    50.0,    // Medium batches
    100.0,   // Standard batch size
    500.0,   // Large batches
    1000.0,  // Very large batches, API limits for some services
    5000.0,  // Huge batches, multi-page operations
    10000.0, // Extremely large operations, multi-request batches
];

/// Buckets for batch operation processing rates (OperationEntriesRate)
/// Measures how many entries can be processed per second
pub const DEFAULT_ENTRIES_RATE_BUCKETS: &[f64] = &[
    1.0,     // Slowest processing, heavy operations per entry
    10.0,    // Slow processing, complex operations
    50.0,    // Moderate processing speed
    100.0,   // Good processing speed, efficient operations
    500.0,   // Fast processing, optimized operations
    1000.0,  // Very fast processing, simple operations
    5000.0,  // Extremely fast processing, bulk operations
    10000.0, // Maximum speed, listing operations, local systems
];

/// Buckets for operation duration metrics like OperationDurationSeconds
/// Covers timeframes from fast metadata operations to long-running transfers
pub const DEFAULT_DURATION_SECONDS_BUCKETS: &[f64] = &[
    0.001, // 1ms - Fastest operations, cached responses
    0.01,  // 10ms - Fast metadata operations, local operations
    0.05,  // 50ms - Quick operations, nearby cloud resources
    0.1,   // 100ms - Standard API response times, typical cloud latency
    0.25,  // 250ms - Medium operations, small data transfers
    0.5,   // 500ms - Medium-long operations, larger metadata operations
    1.0,   // 1s - Long operations, small file transfers
    2.5,   // 2.5s - Extended operations, medium file transfers
    5.0,   // 5s - Long-running operations, large transfers
    10.0,  // 10s - Very long operations, very large transfers
    30.0,  // 30s - Extended operations, complex batch processes
    60.0,  // 1min - Near timeout operations, extremely large transfers
];

/// Buckets for time to first byte metrics like OperationTtfbSeconds
/// Focuses on initial response times, which are typically shorter than full operations
pub const DEFAULT_TTFB_BUCKETS: &[f64] = &[
    0.001, // 1ms - Cached or local resources
    0.01,  // 10ms - Very fast responses, same region
    0.025, // 25ms - Fast responses, optimized configurations
    0.05,  // 50ms - Good response times, standard configurations
    0.1,   // 100ms - Average response times for cloud storage
    0.2,   // 200ms - Slower responses, cross-region or throttled
    0.4,   // 400ms - Poor response times, network congestion
    0.8,   // 800ms - Very slow responses, potential issues
    1.6,   // 1.6s - Problematic responses, retry territory
    3.2,   // 3.2s - Critical latency issues, close to timeouts
];

/// The metric label for the scheme like s3, fs, cos.
pub static LABEL_SCHEME: &str = "scheme";
/// The metric label for the namespace like bucket name in s3.
pub static LABEL_NAMESPACE: &str = "namespace";
/// The metric label for the root path.
pub static LABEL_ROOT: &str = "root";
/// The metric label for the operation like read, write, list.
pub static LABEL_OPERATION: &str = "operation";
/// The metric label for the error.
pub static LABEL_ERROR: &str = "error";
/// The metric label for the http code.
pub static LABEL_STATUS_CODE: &str = "status_code";
/// The metric label for the service-specific operation (e.g., "GetObject", "UploadPart").
pub static LABEL_SERVICE_OPERATION: &str = "service_operation";

/// MetricLabels are the labels for the metrics.
///
/// `scheme`, `namespace`, and `root` always come from the final service stack's
/// [`ServiceInfo`]. HTTP requests only provide request-level labels such as
/// [`Operation`] and [`ServiceOperation`] through extensions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MetricLabels {
    /// The storage scheme identifier (e.g., "s3", "gcs", "azblob", "fs").
    /// Used to differentiate between different storage backends.
    pub scheme: &'static str,
    /// The storage namespace (e.g., bucket name, container name).
    /// Identifies the specific storage container being accessed.
    pub namespace: Arc<str>,
    /// The root path within the namespace that was configured.
    /// Used to track operations within a specific path prefix.
    pub root: Arc<str>,
    /// The operation being performed (e.g., "read", "write", "list").
    /// Identifies which API operation generated this metric.
    pub operation: &'static str,
    /// The specific error kind that occurred during an operation.
    /// Only populated for `OperationErrorsTotal` metric.
    /// Used to track frequency of specific error types.
    pub error: Option<ErrorKind>,
    /// The HTTP status code received in an error response.
    /// Only populated for `HttpStatusErrorsTotal` metric.
    /// Used to track frequency of specific HTTP error status codes.
    pub status_code: Option<StatusCode>,
    /// The service-specific operation name as an optional supplement for operation name.
    pub service_operation: Option<&'static str>,
}

impl MetricLabels {
    /// Create a new set of MetricLabels.
    fn new(info: ServiceInfo, op: &'static str) -> Self {
        MetricLabels {
            scheme: info.scheme(),
            namespace: info.name(),
            root: info.root(),
            operation: op,
            ..MetricLabels::default()
        }
    }

    /// Add error to the metric labels.
    fn with_error(mut self, err: ErrorKind) -> Self {
        self.error = Some(err);
        self
    }

    /// Add status code to the metric labels.
    fn with_status_code(mut self, code: StatusCode) -> Self {
        self.status_code = Some(code);
        self
    }
}

/// MetricValue is the value the opendal sends to the metrics impls.
///
/// Metrics impls can be `prometheus_client`, `metrics` etc.
///
/// Every metrics impls SHOULD implement observe over the MetricValue to make
/// sure they provide the consistent metrics for users.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum MetricValue {
    /// Record the size of data processed in bytes.
    /// Metrics impl: Update a Histogram with the given byte count.
    OperationBytes(u64),
    /// Record the rate of data processing in bytes/second.
    /// Metrics impl: Update a Histogram with the calculated rate value.
    OperationBytesRate(f64),
    /// Record the number of entries (files, objects, keys) processed.
    /// Metrics impl: Update a Histogram with the entry count.
    OperationEntries(u64),
    /// Record the rate of entries processing in entries/second.
    /// Metrics impl: Update a Histogram with the calculated rate value.
    OperationEntriesRate(f64),
    /// Record the total duration of an operation.
    /// Metrics impl: Update a Histogram with the duration converted to seconds (as f64).
    OperationDurationSeconds(Duration),
    /// Increment the counter for operation errors.
    /// Metrics impl: Increment a Counter by 1.
    OperationErrorsTotal,
    /// Update the current number of executing operations.
    /// Metrics impl: Add the value (positive or negative) to a Gauge.
    OperationExecuting(isize),
    /// Record the time to first byte duration.
    /// Metrics impl: Update a Histogram with the duration converted to seconds (as f64).
    OperationTtfbSeconds(Duration),
    /// Update the current number of executing HTTP requests.
    /// Metrics impl: Add the value (positive or negative) to a Gauge.
    HttpExecuting(isize),
    /// Record the size of HTTP request body in bytes.
    /// Metrics impl: Update a Histogram with the given byte count.
    HttpRequestBytes(u64),
    /// Record the rate of HTTP request data in bytes/second.
    /// Metrics impl: Update a Histogram with the calculated rate value.
    HttpRequestBytesRate(f64),
    /// Record the duration of sending an HTTP request (until first byte received).
    /// Metrics impl: Update a Histogram with the duration converted to seconds (as f64).
    HttpRequestDurationSeconds(Duration),
    /// Record the size of HTTP response body in bytes.
    /// Metrics impl: Update a Histogram with the given byte count.
    HttpResponseBytes(u64),
    /// Record the rate of HTTP response data in bytes/second.
    /// Metrics impl: Update a Histogram with the calculated rate value.
    HttpResponseBytesRate(f64),
    /// Record the duration of receiving an HTTP response (from first byte to last).
    /// Metrics impl: Update a Histogram with the duration converted to seconds (as f64).
    HttpResponseDurationSeconds(Duration),
    /// Increment the counter for HTTP connection errors.
    /// Metrics impl: Increment a Counter by 1.
    HttpConnectionErrorsTotal,
    /// Increment the counter for HTTP status errors (non-2xx responses).
    /// Metrics impl: Increment a Counter by 1.
    HttpStatusErrorsTotal,
}

impl MetricValue {
    /// Returns the full metric name for this metric value.
    pub fn name(&self) -> &'static str {
        match self {
            MetricValue::OperationBytes(_) => "opendal_operation_bytes",
            MetricValue::OperationBytesRate(_) => "opendal_operation_bytes_rate",
            MetricValue::OperationEntries(_) => "opendal_operation_entries",
            MetricValue::OperationEntriesRate(_) => "opendal_operation_entries_rate",
            MetricValue::OperationDurationSeconds(_) => "opendal_operation_duration_seconds",
            MetricValue::OperationErrorsTotal => "opendal_operation_errors_total",
            MetricValue::OperationExecuting(_) => "opendal_operation_executing",
            MetricValue::OperationTtfbSeconds(_) => "opendal_operation_ttfb_seconds",

            MetricValue::HttpConnectionErrorsTotal => "opendal_http_connection_errors_total",
            MetricValue::HttpStatusErrorsTotal => "opendal_http_status_errors_total",
            MetricValue::HttpExecuting(_) => "opendal_http_executing",
            MetricValue::HttpRequestBytes(_) => "opendal_http_request_bytes",
            MetricValue::HttpRequestBytesRate(_) => "opendal_http_request_bytes_rate",
            MetricValue::HttpRequestDurationSeconds(_) => "opendal_http_request_duration_seconds",
            MetricValue::HttpResponseBytes(_) => "opendal_http_response_bytes",
            MetricValue::HttpResponseBytesRate(_) => "opendal_http_response_bytes_rate",
            MetricValue::HttpResponseDurationSeconds(_) => "opendal_http_response_duration_seconds",
        }
    }

    /// Returns the metric name along with unit for this metric value.
    ///
    /// # Notes
    ///
    /// This API is designed for the metrics impls that unit aware. They will handle the names by themselves like append `_total` for counters.
    pub fn name_with_unit(&self) -> (&'static str, Option<&'static str>) {
        match self {
            MetricValue::OperationBytes(_) => ("opendal_operation", Some("bytes")),
            MetricValue::OperationBytesRate(_) => ("opendal_operation_bytes_rate", None),
            MetricValue::OperationEntries(_) => ("opendal_operation_entries", None),
            MetricValue::OperationEntriesRate(_) => ("opendal_operation_entries_rate", None),
            MetricValue::OperationDurationSeconds(_) => {
                ("opendal_operation_duration", Some("seconds"))
            }
            MetricValue::OperationErrorsTotal => ("opendal_operation_errors", None),
            MetricValue::OperationExecuting(_) => ("opendal_operation_executing", None),
            MetricValue::OperationTtfbSeconds(_) => ("opendal_operation_ttfb", Some("seconds")),

            MetricValue::HttpConnectionErrorsTotal => ("opendal_http_connection_errors", None),
            MetricValue::HttpStatusErrorsTotal => ("opendal_http_status_errors", None),
            MetricValue::HttpExecuting(_) => ("opendal_http_executing", None),
            MetricValue::HttpRequestBytes(_) => ("opendal_http_request", Some("bytes")),
            MetricValue::HttpRequestBytesRate(_) => ("opendal_http_request_bytes_rate", None),
            MetricValue::HttpRequestDurationSeconds(_) => {
                ("opendal_http_request_duration", Some("seconds"))
            }
            MetricValue::HttpResponseBytes(_) => ("opendal_http_response", Some("bytes")),
            MetricValue::HttpResponseBytesRate(_) => ("opendal_http_response_bytes_rate", None),
            MetricValue::HttpResponseDurationSeconds(_) => {
                ("opendal_http_response_duration", Some("seconds"))
            }
        }
    }

    /// Returns the help text for this metric value.
    pub fn help(&self) -> &'static str {
        match self {
            MetricValue::OperationBytes(_) => {
                "Current operation size in bytes, represents the size of data being processed in the current operation"
            }
            MetricValue::OperationBytesRate(_) => {
                "Histogram of data processing rates in bytes per second within individual operations"
            }
            MetricValue::OperationEntries(_) => {
                "Current operation size in entries, represents the entries being processed in the current operation"
            }
            MetricValue::OperationEntriesRate(_) => {
                "Histogram of entries processing rates in entries per second within individual operations"
            }
            MetricValue::OperationDurationSeconds(_) => {
                "Duration of operations in seconds, measured from start to completion"
            }
            MetricValue::OperationErrorsTotal => "Total number of failed operations",
            MetricValue::OperationExecuting(_) => "Number of operations currently being executed",
            MetricValue::OperationTtfbSeconds(_) => "Time to first byte in seconds for operations",

            MetricValue::HttpConnectionErrorsTotal => {
                "Total number of HTTP requests that failed before receiving a response (DNS failures, connection refused, timeouts, TLS errors)"
            }
            MetricValue::HttpStatusErrorsTotal => {
                "Total number of HTTP requests that received error status codes (non-2xx responses)"
            }
            MetricValue::HttpExecuting(_) => {
                "Number of HTTP requests currently in flight from this client"
            }
            MetricValue::HttpRequestBytes(_) => "Histogram of HTTP request body sizes in bytes",
            MetricValue::HttpRequestBytesRate(_) => {
                "Histogram of HTTP request bytes per second rates"
            }
            MetricValue::HttpRequestDurationSeconds(_) => {
                "Histogram of time durations in seconds spent sending HTTP requests, from first byte sent to receiving the first byte"
            }
            MetricValue::HttpResponseBytes(_) => "Histogram of HTTP response body sizes in bytes",
            MetricValue::HttpResponseBytesRate(_) => {
                "Histogram of HTTP response bytes per second rates"
            }
            MetricValue::HttpResponseDurationSeconds(_) => {
                "Histogram of time durations in seconds spent receiving HTTP responses, from first byte received to last byte received"
            }
        }
    }
}

/// The interceptor for metrics.
///
/// All metrics related libs should implement this trait to observe opendal's internal operations.
pub trait MetricsIntercept: Debug + Clone + Send + Sync + Unpin + 'static {
    /// Observe the metric value.
    fn observe(&self, labels: MetricLabels, value: MetricValue) {
        let _ = (labels, value);
    }
}

/// `MetricsLayer` reports operation and HTTP metrics to an interceptor.
///
/// The layer wraps both the storage service and the HTTP transport in the
/// operator's [`OperationContext`]. It reports the metric names and labels
/// documented in the crate-level metrics reference through
/// [`MetricsIntercept::observe`].
///
/// The interceptor must be cheap to clone because each wrapped operation and
/// HTTP request can retain a clone.
#[derive(Clone, Debug)]
pub struct MetricsLayer<I: MetricsIntercept> {
    interceptor: I,
}

impl<I: MetricsIntercept> MetricsLayer<I> {
    /// Create a metrics layer that reports values to `interceptor`.
    pub fn new(interceptor: I) -> Self {
        Self { interceptor }
    }
}

impl<I: MetricsIntercept> Layer for MetricsLayer<I> {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }

    fn apply_context(&self, srv: Servicer, inner: OperationContext) -> OperationContext {
        // HTTP metrics share the same interceptor and service identity as operation metrics.
        let transport = HttpTransporter::new(MetricsHttpTransport {
            inner: inner.http_transport().clone(),
            info: srv.info(),
            interceptor: self.interceptor.clone(),
        });
        inner.with_http_transport(transport)
    }
}

impl<I: MetricsIntercept> MetricsLayer<I> {
    fn layer(&self, inner: Servicer) -> MetricsAccessor<I> {
        let info = inner.info();
        MetricsAccessor {
            inner,
            info,
            interceptor: self.interceptor.clone(),
        }
    }
}

struct MetricsHttpTransport<I: MetricsIntercept> {
    inner: HttpTransporter,
    info: ServiceInfo,
    interceptor: I,
}

enum ExecutingGuardKind {
    Http,
    Operation,
}

/// Guard to ensure metrics accounted even if the async future is cancelled.
struct ExecutingGuard<I: MetricsIntercept> {
    /// The interceptor to use for recording metrics. `None` if defused.
    interceptor: Option<I>,
    labels: MetricLabels,
    start: Instant,
    kind: ExecutingGuardKind,
    /// Whether the async operation finished.
    completed: bool,
}

impl<I: MetricsIntercept> ExecutingGuard<I> {
    fn new_http(interceptor: I, labels: MetricLabels, start: Instant) -> Self {
        Self {
            interceptor: Some(interceptor),
            labels,
            start,
            kind: ExecutingGuardKind::Http,
            completed: false,
        }
    }

    fn new_operation(interceptor: I, labels: MetricLabels, start: Instant) -> Self {
        Self {
            interceptor: Some(interceptor),
            labels,
            start,
            kind: ExecutingGuardKind::Operation,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }

    fn defuse(&mut self) {
        self.interceptor = None;
    }
}

impl<I: MetricsIntercept> Drop for ExecutingGuard<I> {
    fn drop(&mut self) {
        let Some(ref interceptor) = self.interceptor else {
            return;
        };

        if !self.completed {
            match self.kind {
                ExecutingGuardKind::Http => {
                    interceptor
                        .observe(self.labels.clone(), MetricValue::HttpConnectionErrorsTotal);
                    interceptor.observe(
                        self.labels.clone(),
                        MetricValue::HttpRequestDurationSeconds(self.start.elapsed()),
                    );
                }
                ExecutingGuardKind::Operation => {
                    interceptor.observe(
                        self.labels.clone().with_error(ErrorKind::Unexpected),
                        MetricValue::OperationErrorsTotal,
                    );
                    interceptor.observe(
                        self.labels.clone(),
                        MetricValue::OperationDurationSeconds(self.start.elapsed()),
                    );
                }
            }
        }

        match self.kind {
            ExecutingGuardKind::Http => {
                interceptor.observe(
                    std::mem::take(&mut self.labels),
                    MetricValue::HttpExecuting(-1),
                );
            }
            ExecutingGuardKind::Operation => {
                interceptor.observe(
                    std::mem::take(&mut self.labels),
                    MetricValue::OperationExecuting(-1),
                );
            }
        }
    }
}

impl<I: MetricsIntercept> HttpTransport for MetricsHttpTransport<I> {
    async fn fetch(&self, req: http::Request<Buffer>) -> Result<http::Response<HttpBody>> {
        let mut labels = MetricLabels::new(
            self.info.clone(),
            req.extensions()
                .get::<Operation>()
                .copied()
                .map(Operation::into_static)
                .unwrap_or("unknown"),
        );
        labels.service_operation = req.extensions().get::<ServiceOperation>().map(|s| s.0);

        let start = Instant::now();
        let req_size = req.body().len();

        self.interceptor
            .observe(labels.clone(), MetricValue::HttpExecuting(1));
        let mut guard = ExecutingGuard::new_http(self.interceptor.clone(), labels.clone(), start);

        let res = self.inner.fetch(req).await;
        guard.complete();
        let req_duration = start.elapsed();

        match res {
            Err(err) => {
                self.interceptor
                    .observe(labels, MetricValue::HttpConnectionErrorsTotal);
                Err(err)
            }
            Ok(resp) if resp.status().is_client_error() || resp.status().is_server_error() => {
                self.interceptor.observe(
                    labels.with_status_code(resp.status()),
                    MetricValue::HttpStatusErrorsTotal,
                );
                Ok(resp)
            }
            Ok(resp) => {
                guard.defuse();
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::HttpRequestBytes(req_size as u64),
                );
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::HttpRequestBytesRate(req_size as f64 / req_duration.as_secs_f64()),
                );
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::HttpRequestDurationSeconds(req_duration),
                );

                let (parts, body) = resp.into_parts();
                let body = body.map_inner(|s| {
                    Box::new(MetricsStream {
                        inner: s,
                        interceptor: self.interceptor.clone(),
                        labels: labels.clone(),
                        size: 0,
                        start: Instant::now(),
                        succeeded: false,
                    })
                });

                Ok(http::Response::from_parts(parts, body))
            }
        }
    }
}

struct MetricsStream<S, I: MetricsIntercept> {
    inner: S,
    interceptor: I,

    labels: MetricLabels,
    size: u64,
    start: Instant,
    succeeded: bool,
}

impl<S, I> Stream for MetricsStream<S, I>
where
    S: Stream<Item = Result<Buffer>> + Unpin + 'static,
    I: MetricsIntercept,
{
    type Item = Result<Buffer>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match ready!(self.inner.poll_next_unpin(cx)) {
            Some(Ok(bs)) => {
                self.size += bs.len() as u64;
                Poll::Ready(Some(Ok(bs)))
            }
            Some(Err(err)) => Poll::Ready(Some(Err(err))),
            None => {
                self.succeeded = true;
                Poll::Ready(None)
            }
        }
    }
}

impl<S, I: MetricsIntercept> Drop for MetricsStream<S, I> {
    fn drop(&mut self) {
        let resp_size = self.size;
        let resp_duration = self.start.elapsed();

        if !self.succeeded {
            self.interceptor
                .observe(self.labels.clone(), MetricValue::HttpConnectionErrorsTotal);
        }

        self.interceptor.observe(
            self.labels.clone(),
            MetricValue::HttpResponseBytes(resp_size),
        );
        self.interceptor.observe(
            self.labels.clone(),
            MetricValue::HttpResponseBytesRate(resp_size as f64 / resp_duration.as_secs_f64()),
        );
        self.interceptor.observe(
            self.labels.clone(),
            MetricValue::HttpResponseDurationSeconds(resp_duration),
        );
        self.interceptor
            .observe(self.labels.clone(), MetricValue::HttpExecuting(-1));
    }
}

#[doc(hidden)]
pub struct MetricsAccessor<I: MetricsIntercept> {
    inner: Servicer,
    info: ServiceInfo,
    interceptor: I,
}

impl<I: MetricsIntercept> Debug for MetricsAccessor<I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsAccessor")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<I: MetricsIntercept> Service for MetricsAccessor<I> {
    type Reader = MetricsReader<oio::Reader, I>;
    type Writer = MetricsWrapper<oio::Writer, I>;
    type Lister = MetricsWrapper<oio::Lister, I>;
    type Deleter = MetricsWrapper<oio::Deleter, I>;
    type Copier = MetricsWrapper<oio::Copier, I>;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.info.clone()
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
        let labels = MetricLabels::new(self.info.clone(), Operation::CreateDir.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        let res = self
            .inner
            .create_dir(ctx, path, args)
            .await
            .inspect(|_| {
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::OperationDurationSeconds(start.elapsed()),
                );
            })
            .inspect_err(|err| {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
            });

        guard.complete();
        res
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let labels = MetricLabels::new(self.info.clone(), Operation::Read.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        match self.inner.read(ctx, path, args) {
            Ok(reader) => {
                guard.complete();
                Ok(MetricsReader::new(reader, self.interceptor.clone(), labels))
            }
            Err(err) => {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
                guard.complete();
                Err(err)
            }
        }
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let labels = MetricLabels::new(self.info.clone(), Operation::Write.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        match self.inner.write(ctx, path, args) {
            Ok(writer) => {
                guard.defuse();
                Ok(MetricsWrapper::new(
                    writer,
                    self.interceptor.clone(),
                    labels,
                    start,
                ))
            }
            Err(err) => {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
                guard.complete();
                Err(err)
            }
        }
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let labels = MetricLabels::new(self.info.clone(), Operation::Copy.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        match self.inner.copy(ctx, from, to, args) {
            Ok(copier) => {
                guard.defuse();
                Ok(MetricsWrapper::new(
                    copier,
                    self.interceptor.clone(),
                    labels,
                    start,
                ))
            }
            Err(err) => {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
                guard.complete();
                Err(err)
            }
        }
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        let labels = MetricLabels::new(self.info.clone(), Operation::Rename.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        let res = self
            .inner
            .rename(ctx, from, to, args)
            .await
            .inspect(|_| {
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::OperationDurationSeconds(start.elapsed()),
                );
            })
            .inspect_err(|err| {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
            });

        guard.complete();
        res
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        let labels = MetricLabels::new(self.info.clone(), Operation::Restore.into_static());
        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        let res = self
            .inner
            .restore(ctx, path, args)
            .await
            .inspect(|_| {
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::OperationDurationSeconds(start.elapsed()),
                );
            })
            .inspect_err(|err| {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
            });

        guard.complete();
        res
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let labels = MetricLabels::new(self.info.clone(), Operation::Stat.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        let res = self
            .inner
            .stat(ctx, path, args)
            .await
            .inspect(|_| {
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::OperationDurationSeconds(start.elapsed()),
                );
            })
            .inspect_err(|err| {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
            });

        guard.complete();
        res
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        let labels = MetricLabels::new(self.info.clone(), Operation::Delete.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        match self.inner.delete(ctx) {
            Ok(deleter) => {
                guard.defuse();
                Ok(MetricsWrapper::new(
                    deleter,
                    self.interceptor.clone(),
                    labels,
                    start,
                ))
            }
            Err(err) => {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
                guard.complete();
                Err(err)
            }
        }
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let labels = MetricLabels::new(self.info.clone(), Operation::List.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        match self.inner.list(ctx, path, args) {
            Ok(lister) => {
                guard.defuse();
                Ok(MetricsWrapper::new(
                    lister,
                    self.interceptor.clone(),
                    labels,
                    start,
                ))
            }
            Err(err) => {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
                guard.complete();
                Err(err)
            }
        }
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let labels = MetricLabels::new(self.info.clone(), Operation::Presign.into_static());

        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        let res = self
            .inner
            .presign(ctx, path, args)
            .await
            .inspect(|_| {
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::OperationDurationSeconds(start.elapsed()),
                );
            })
            .inspect_err(|err| {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
            });

        guard.complete();
        res
    }
}

#[doc(hidden)]
pub struct MetricsReader<R, I: MetricsIntercept> {
    inner: R,
    interceptor: I,
    labels: MetricLabels,
}

impl<R, I: MetricsIntercept> MetricsReader<R, I> {
    fn new(inner: R, interceptor: I, labels: MetricLabels) -> Self {
        Self {
            inner,
            interceptor,
            labels,
        }
    }
}

impl<R: oio::Read, I: MetricsIntercept> oio::Read for MetricsReader<R, I> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let labels = self.labels.clone();
        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard =
            ExecutingGuard::new_operation(self.interceptor.clone(), labels.clone(), start);

        match self.inner.open(range).await {
            Ok((rp, stream)) => {
                self.interceptor.observe(
                    labels.clone(),
                    MetricValue::OperationTtfbSeconds(start.elapsed()),
                );
                guard.defuse();
                Ok((
                    rp,
                    Box::new(MetricsWrapper::new(
                        stream,
                        self.interceptor.clone(),
                        labels,
                        start,
                    )) as Box<dyn oio::ReadStreamDyn>,
                ))
            }
            Err(err) => {
                self.interceptor.observe(
                    labels.clone().with_error(err.kind()),
                    MetricValue::OperationErrorsTotal,
                );
                guard.complete();
                Err(err)
            }
        }
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let labels = self.labels.clone();
        let start = Instant::now();

        self.interceptor
            .observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut metrics = MetricsWrapper::new((), self.interceptor.clone(), labels, start);

        let result = self.inner.read(range).await.inspect_err(|err| {
            metrics.record_error(err);
        });
        if let Ok((_, buffer)) = &result {
            metrics.size = buffer.len() as u64;
            metrics.completed = true;
        }
        result
    }
}

#[doc(hidden)]
pub struct MetricsWrapper<R, I: MetricsIntercept> {
    inner: R,
    interceptor: I,
    labels: MetricLabels,

    start: Instant,
    size: u64,
    completed: bool,
}

impl<R, I: MetricsIntercept> Drop for MetricsWrapper<R, I> {
    fn drop(&mut self) {
        let size = self.size;
        let duration = self.start.elapsed();

        if !self.completed {
            self.interceptor.observe(
                self.labels.clone().with_error(ErrorKind::Unexpected),
                MetricValue::OperationErrorsTotal,
            );
        }

        if self.labels.operation == Operation::Read.into_static()
            || self.labels.operation == Operation::Write.into_static()
            || self.labels.operation == Operation::Copy.into_static()
        {
            self.interceptor
                .observe(self.labels.clone(), MetricValue::OperationBytes(self.size));
            self.interceptor.observe(
                self.labels.clone(),
                MetricValue::OperationBytesRate(size as f64 / duration.as_secs_f64()),
            );
        } else {
            self.interceptor.observe(
                self.labels.clone(),
                MetricValue::OperationEntries(self.size),
            );
            self.interceptor.observe(
                self.labels.clone(),
                MetricValue::OperationEntriesRate(size as f64 / duration.as_secs_f64()),
            );
        }

        self.interceptor.observe(
            self.labels.clone(),
            MetricValue::OperationDurationSeconds(duration),
        );
        self.interceptor
            .observe(self.labels.clone(), MetricValue::OperationExecuting(-1));
    }
}

impl<R, I: MetricsIntercept> MetricsWrapper<R, I> {
    fn new(inner: R, interceptor: I, labels: MetricLabels, start: Instant) -> Self {
        Self {
            inner,
            interceptor,
            labels,
            start,
            size: 0,
            completed: false,
        }
    }

    fn record_error(&mut self, err: &Error) {
        self.completed = true;
        self.interceptor.observe(
            self.labels.clone().with_error(err.kind()),
            MetricValue::OperationErrorsTotal,
        );
    }
}

impl<R: oio::ReadStream, I: MetricsIntercept> oio::ReadStream for MetricsWrapper<R, I> {
    async fn read(&mut self) -> Result<Buffer> {
        self.inner
            .read()
            .await
            .inspect(|bs| {
                if bs.is_empty() {
                    self.completed = true;
                }
                self.size += bs.len() as u64;
            })
            .inspect_err(|err| self.record_error(err))
    }
}

impl<R: oio::Write, I: MetricsIntercept> oio::Write for MetricsWrapper<R, I> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let size = bs.len();

        self.inner
            .write(bs)
            .await
            .inspect(|_| {
                self.size += size as u64;
            })
            .inspect_err(|err| self.record_error(err))
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        let size = range
            .size()
            .expect("writer copy range must be absolute and bounded");

        self.inner
            .copy_from(path, args, range)
            .await
            .inspect(|_| {
                self.size += size;
            })
            .inspect_err(|err| self.record_error(err))
    }

    async fn close(&mut self) -> Result<Metadata> {
        let result = self
            .inner
            .close()
            .await
            .inspect_err(|err| self.record_error(err));
        self.completed = true;
        result
    }

    async fn abort(&mut self) -> Result<()> {
        let result = self
            .inner
            .abort()
            .await
            .inspect_err(|err| self.record_error(err));
        self.completed = true;
        result
    }
}

impl<R: oio::List, I: MetricsIntercept> oio::List for MetricsWrapper<R, I> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.inner
            .next()
            .await
            .inspect(|entry| {
                if entry.is_some() {
                    self.size += 1;
                } else {
                    self.completed = true;
                }
            })
            .inspect_err(|err| self.record_error(err))
    }
}

impl<R: oio::Delete, I: MetricsIntercept> oio::Delete for MetricsWrapper<R, I> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.inner
            .delete(path, args)
            .await
            .inspect(|_| {
                self.size += 1;
            })
            .inspect_err(|err| self.record_error(err))
    }

    async fn close(&mut self) -> Result<()> {
        let result = self
            .inner
            .close()
            .await
            .inspect_err(|err| self.record_error(err));
        self.completed = true;
        result
    }
}

impl<C: oio::Copy, I: MetricsIntercept> oio::Copy for MetricsWrapper<C, I> {
    async fn next(&mut self) -> Result<Option<usize>> {
        self.inner
            .next()
            .await
            .inspect(|n| match n {
                Some(n) => self.size += *n as u64,
                None => self.completed = true,
            })
            .inspect_err(|err| self.record_error(err))
    }

    async fn close(&mut self) -> Result<Metadata> {
        let result = self
            .inner
            .close()
            .await
            .inspect_err(|err| self.record_error(err));
        self.completed = true;
        result
    }

    async fn abort(&mut self) -> Result<()> {
        let result = self
            .inner
            .abort()
            .await
            .inspect_err(|err| self.record_error(err));
        self.completed = true;
        result
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::time::Instant;

    use futures::StreamExt;
    use futures::stream;

    use super::*;

    #[derive(Debug, Clone, Default)]
    struct MockInterceptor {
        observations: Arc<Mutex<Vec<(MetricLabels, MetricValue)>>>,
    }

    impl MetricsIntercept for MockInterceptor {
        fn observe(&self, labels: MetricLabels, value: MetricValue) {
            self.observations.lock().unwrap().push((labels, value));
        }
    }

    impl MockInterceptor {
        fn has_metric(&self, name: &str) -> bool {
            self.observations
                .lock()
                .unwrap()
                .iter()
                .any(|(_, v)| v.name() == name)
        }

        fn count_metric(&self, name: &str) -> usize {
            self.observations
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, v)| v.name() == name)
                .count()
        }

        fn count_metric_with_status(&self, name: &str, code: StatusCode) -> usize {
            self.observations
                .lock()
                .unwrap()
                .iter()
                .filter(|(l, v)| v.name() == name && l.status_code == Some(code))
                .count()
        }

        fn gauge_value(&self, name: &str) -> isize {
            self.observations
                .lock()
                .unwrap()
                .iter()
                .filter_map(|(_, v)| match v {
                    MetricValue::HttpExecuting(n) if v.name() == name => Some(*n),
                    MetricValue::OperationExecuting(n) if v.name() == name => Some(*n),
                    _ => None,
                })
                .sum()
        }

        fn get_duration_seconds(&self, name: &str) -> Option<Duration> {
            self.observations.lock().unwrap().iter().find_map(|(_, v)| {
                if v.name() != name {
                    return None;
                }
                match v {
                    MetricValue::HttpRequestDurationSeconds(d)
                    | MetricValue::HttpResponseDurationSeconds(d)
                    | MetricValue::OperationDurationSeconds(d)
                    | MetricValue::OperationTtfbSeconds(d) => Some(*d),
                    _ => None,
                }
            })
        }

        fn get_value_u64(&self, name: &str) -> Option<u64> {
            self.observations.lock().unwrap().iter().find_map(|(_, v)| {
                if v.name() != name {
                    return None;
                }
                match v {
                    MetricValue::HttpResponseBytes(n)
                    | MetricValue::HttpRequestBytes(n)
                    | MetricValue::OperationBytes(n)
                    | MetricValue::OperationEntries(n) => Some(*n),
                    _ => None,
                }
            })
        }
    }

    fn test_info() -> ServiceInfo {
        ServiceInfo::new("test", "", "")
    }

    fn test_labels() -> MetricLabels {
        MetricLabels::new(test_info(), Operation::Read.into_static())
    }

    enum MockFetchBehavior {
        /// Return a response with the given status code and an empty body.
        Respond(StatusCode),
        /// Fail before receiving a response.
        ConnectionError,
        /// Return HTTP 200 with a body stream that yields the given items.
        StreamBody(Vec<Result<Buffer>>),
    }

    struct MockHttpTransport {
        behavior: Mutex<MockFetchBehavior>,
    }

    impl HttpTransport for MockHttpTransport {
        async fn fetch(&self, _req: http::Request<Buffer>) -> Result<http::Response<HttpBody>> {
            let behavior = std::mem::replace(
                &mut *self.behavior.lock().unwrap(),
                MockFetchBehavior::ConnectionError,
            );
            match behavior {
                MockFetchBehavior::Respond(status) => {
                    let body = HttpBody::new(stream::empty(), None);
                    let resp = http::Response::builder().status(status).body(body).unwrap();
                    Ok(resp)
                }
                MockFetchBehavior::ConnectionError => {
                    Err(Error::new(ErrorKind::Unexpected, "mock connection refused"))
                }
                MockFetchBehavior::StreamBody(items) => {
                    let body = HttpBody::new(stream::iter(items), None);
                    let resp = http::Response::builder()
                        .status(StatusCode::OK)
                        .body(body)
                        .unwrap();
                    Ok(resp)
                }
            }
        }
    }

    fn build_metrics_http_fetcher(
        mock: MockInterceptor,
        behavior: MockFetchBehavior,
    ) -> MetricsHttpTransport<MockInterceptor> {
        let inner_fetch = MockHttpTransport {
            behavior: Mutex::new(behavior),
        };
        MetricsHttpTransport {
            inner: HttpTransporter::new(inner_fetch),
            info: test_info(),
            interceptor: mock,
        }
    }

    fn build_http_request() -> http::Request<Buffer> {
        let mut req = http::Request::new(Buffer::new());
        *req.uri_mut() = "https://example.com/test".parse().unwrap();
        req.extensions_mut().insert(Operation::Read);
        req
    }

    struct MockRawReader {
        open_result: Mutex<Option<Result<Vec<Result<Buffer>>>>>,
        read_result: Mutex<Option<Result<Buffer>>>,
    }

    impl MockRawReader {
        fn with_open(chunks: Vec<Result<Buffer>>) -> Self {
            Self {
                open_result: Mutex::new(Some(Ok(chunks))),
                read_result: Mutex::new(None),
            }
        }

        fn with_read(buffer: Buffer) -> Self {
            Self {
                open_result: Mutex::new(None),
                read_result: Mutex::new(Some(Ok(buffer))),
            }
        }
    }

    impl oio::Read for MockRawReader {
        async fn open(&self, _: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
            match self
                .open_result
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Err(Error::new(ErrorKind::Unexpected, "open not configured")))
            {
                Ok(chunks) => Ok((
                    RpRead::default(),
                    Box::new(MockReadStream {
                        chunks: chunks.into_iter().collect(),
                    }) as Box<dyn oio::ReadStreamDyn>,
                )),
                Err(err) => Err(err),
            }
        }

        async fn read(&self, _: BytesRange) -> Result<(RpRead, Buffer)> {
            match self
                .read_result
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Err(Error::new(ErrorKind::Unexpected, "read not configured")))
            {
                Ok(buffer) => Ok((RpRead::default(), buffer)),
                Err(err) => Err(err),
            }
        }
    }

    struct MockReadStream {
        chunks: VecDeque<Result<Buffer>>,
    }

    impl oio::ReadStream for MockReadStream {
        async fn read(&mut self) -> Result<Buffer> {
            self.chunks.pop_front().unwrap_or_else(|| Ok(Buffer::new()))
        }
    }

    #[tokio::test]
    async fn test_http_transport_error_records_status_error() {
        let mock = MockInterceptor::default();
        let fetcher = build_metrics_http_fetcher(
            mock.clone(),
            MockFetchBehavior::Respond(StatusCode::NOT_FOUND),
        );

        let _ = fetcher.fetch(build_http_request()).await;

        assert_eq!(
            mock.count_metric_with_status(
                "opendal_http_status_errors_total",
                StatusCode::NOT_FOUND
            ),
            1
        );
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
    }

    #[tokio::test]
    async fn test_http_server_error_records_status_error() {
        let mock = MockInterceptor::default();
        let fetcher = build_metrics_http_fetcher(
            mock.clone(),
            MockFetchBehavior::Respond(StatusCode::INTERNAL_SERVER_ERROR),
        );

        let _ = fetcher.fetch(build_http_request()).await;

        assert_eq!(
            mock.count_metric_with_status(
                "opendal_http_status_errors_total",
                StatusCode::INTERNAL_SERVER_ERROR
            ),
            1
        );
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
    }

    #[tokio::test]
    async fn test_http_success_records_full_metrics() {
        let mock = MockInterceptor::default();
        let fetcher = build_metrics_http_fetcher(
            mock.clone(),
            MockFetchBehavior::StreamBody(vec![
                Ok(Buffer::from("hello")),
                Ok(Buffer::from(" world")),
            ]),
        );

        let resp = fetcher.fetch(build_http_request()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let (_, mut body) = resp.into_parts();
        let buf = body.to_buffer().await.unwrap();
        assert_eq!(buf.len(), 11);
        drop(body);

        assert_eq!(mock.count_metric("opendal_http_status_errors_total"), 0);
        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 0);
        assert_eq!(mock.get_value_u64("opendal_http_request_bytes"), Some(0));
        assert!(
            mock.get_duration_seconds("opendal_http_request_duration_seconds")
                .unwrap()
                > Duration::ZERO
        );
        assert_eq!(mock.get_value_u64("opendal_http_response_bytes"), Some(11));
        assert!(
            mock.get_duration_seconds("opendal_http_response_duration_seconds")
                .unwrap()
                > Duration::ZERO
        );
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
    }

    #[tokio::test]
    async fn test_http_connection_error_records_connection_error() {
        let mock = MockInterceptor::default();
        let fetcher = build_metrics_http_fetcher(mock.clone(), MockFetchBehavior::ConnectionError);

        let res = fetcher.fetch(build_http_request()).await;
        assert!(res.is_err());

        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 1);
        assert_eq!(mock.count_metric("opendal_http_status_errors_total"), 0);
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
    }

    #[tokio::test]
    async fn test_http_body_read_failure_records_metrics() {
        let mock = MockInterceptor::default();
        let fetcher = build_metrics_http_fetcher(
            mock.clone(),
            MockFetchBehavior::StreamBody(vec![
                Ok(Buffer::from("partial")),
                Err(Error::new(ErrorKind::Unexpected, "connection reset")),
            ]),
        );

        let resp = fetcher.fetch(build_http_request()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let (_, mut body) = resp.into_parts();
        let res = body.to_buffer().await;
        assert!(res.is_err());
        drop(body);

        assert_eq!(mock.get_value_u64("opendal_http_response_bytes"), Some(7));
        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 1);
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
    }

    #[tokio::test]
    async fn test_stream_completed_records_response_metrics() {
        let mock = MockInterceptor::default();
        let labels = test_labels();
        mock.observe(labels.clone(), MetricValue::HttpExecuting(1));
        let inner = stream::iter(vec![Ok(Buffer::from("hello"))]);

        let mut s = MetricsStream {
            inner,
            interceptor: mock.clone(),
            labels,
            size: 0,
            start: Instant::now(),
            succeeded: false,
        };

        while s.next().await.is_some() {}
        drop(s);

        assert_eq!(mock.get_value_u64("opendal_http_response_bytes"), Some(5));
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 0);
    }

    #[tokio::test]
    async fn test_stream_dropped_early_records_response_metrics() {
        let mock = MockInterceptor::default();
        let labels = test_labels();
        mock.observe(labels.clone(), MetricValue::HttpExecuting(1));
        let inner = stream::iter(vec![Ok(Buffer::from("chunk1")), Ok(Buffer::from("chunk2"))]);

        let mut s = MetricsStream {
            inner,
            interceptor: mock.clone(),
            labels,
            size: 0,
            start: Instant::now(),
            succeeded: false,
        };

        let _ = s.next().await;
        drop(s);

        assert_eq!(mock.get_value_u64("opendal_http_response_bytes"), Some(6));
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 1);
    }

    #[tokio::test]
    async fn test_stream_error_records_response_metrics() {
        let mock = MockInterceptor::default();
        let labels = test_labels();
        mock.observe(labels.clone(), MetricValue::HttpExecuting(1));
        let inner = stream::iter(vec![
            Ok(Buffer::from("data")),
            Err(Error::new(ErrorKind::Unexpected, "read error")),
        ]);

        let mut s = MetricsStream {
            inner,
            interceptor: mock.clone(),
            labels,
            size: 0,
            start: Instant::now(),
            succeeded: false,
        };

        while let Some(res) = s.next().await {
            if res.is_err() {
                break;
            }
        }
        drop(s);

        assert_eq!(mock.get_value_u64("opendal_http_response_bytes"), Some(4));
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 1);
    }

    #[test]
    fn test_http_guard_cancelled_records_connection_error() {
        let mock = MockInterceptor::default();
        let labels = test_labels();
        mock.observe(labels.clone(), MetricValue::HttpExecuting(1));
        let guard = ExecutingGuard::new_http(mock.clone(), labels, Instant::now());
        drop(guard);

        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 1);
        assert!(mock.has_metric("opendal_http_request_duration_seconds"));
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
    }

    #[test]
    fn test_http_guard_completed_only_decrements_executing() {
        let mock = MockInterceptor::default();
        let labels = test_labels();
        mock.observe(labels.clone(), MetricValue::HttpExecuting(1));
        let mut guard = ExecutingGuard::new_http(mock.clone(), labels, Instant::now());
        guard.complete();
        drop(guard);

        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 0);
        assert_eq!(mock.gauge_value("opendal_http_executing"), 0);
    }

    #[test]
    fn test_http_guard_defused_records_nothing() {
        let mock = MockInterceptor::default();
        let labels = test_labels();
        mock.observe(labels.clone(), MetricValue::HttpExecuting(1));
        let mut guard = ExecutingGuard::new_http(mock.clone(), labels, Instant::now());
        guard.complete();
        guard.defuse();
        drop(guard);

        assert_eq!(mock.gauge_value("opendal_http_executing"), 1);
        assert_eq!(mock.count_metric("opendal_http_connection_errors_total"), 0);
    }

    #[test]
    fn test_operation_guard_cancelled_records_operation_error() {
        let mock = MockInterceptor::default();
        let labels = test_labels();
        mock.observe(labels.clone(), MetricValue::OperationExecuting(1));
        let guard = ExecutingGuard::new_operation(mock.clone(), labels, Instant::now());
        drop(guard);

        assert_eq!(mock.count_metric("opendal_operation_errors_total"), 1);
        assert!(mock.has_metric("opendal_operation_duration_seconds"));
        assert_eq!(mock.gauge_value("opendal_operation_executing"), 0);
    }

    #[test]
    fn test_operation_guard_completed_only_decrements_executing() {
        let mock = MockInterceptor::default();
        let labels = test_labels();
        mock.observe(labels.clone(), MetricValue::OperationExecuting(1));
        let mut guard = ExecutingGuard::new_operation(mock.clone(), labels, Instant::now());
        guard.complete();
        drop(guard);

        assert_eq!(mock.count_metric("opendal_operation_errors_total"), 0);
        assert_eq!(mock.gauge_value("opendal_operation_executing"), 0);
    }

    #[test]
    fn test_reader_drop_records_no_operation_metrics() {
        let mock = MockInterceptor::default();
        let labels = MetricLabels::new(test_info(), Operation::Read.into_static());

        let reader = MetricsReader::new(
            MockRawReader::with_read(Buffer::new()),
            mock.clone(),
            labels,
        );
        drop(reader);

        assert_eq!(mock.count_metric("opendal_operation_errors_total"), 0);
        assert_eq!(mock.get_value_u64("opendal_operation_bytes"), None);
        assert_eq!(mock.gauge_value("opendal_operation_executing"), 0);
    }

    #[tokio::test]
    async fn test_reader_open_stream_records_operation_metrics_on_stream_drop() {
        let mock = MockInterceptor::default();
        let labels = MetricLabels::new(test_info(), Operation::Read.into_static());
        let reader = MetricsReader::new(
            MockRawReader::with_open(vec![Ok(Buffer::from("hello"))]),
            mock.clone(),
            labels,
        );

        let (_, mut stream) = oio::Read::open(&reader, BytesRange::from(0_u64..5))
            .await
            .unwrap();
        assert_eq!(mock.gauge_value("opendal_operation_executing"), 1);

        let chunk = oio::ReadStream::read(&mut stream).await.unwrap();
        assert_eq!(chunk.len(), 5);
        let chunk = oio::ReadStream::read(&mut stream).await.unwrap();
        assert!(chunk.is_empty());
        drop(stream);
        drop(reader);

        assert_eq!(mock.count_metric("opendal_operation_errors_total"), 0);
        assert!(mock.has_metric("opendal_operation_ttfb_seconds"));
        assert_eq!(mock.get_value_u64("opendal_operation_bytes"), Some(5));
        assert_eq!(mock.gauge_value("opendal_operation_executing"), 0);
    }

    #[tokio::test]
    async fn test_reader_read_records_operation_metrics() {
        let mock = MockInterceptor::default();
        let labels = MetricLabels::new(test_info(), Operation::Read.into_static());
        let reader = MetricsReader::new(
            MockRawReader::with_read(Buffer::from("hello")),
            mock.clone(),
            labels,
        );

        let (_, buffer) = oio::Read::read(&reader, BytesRange::from(0_u64..5))
            .await
            .unwrap();
        drop(reader);

        assert_eq!(buffer.len(), 5);
        assert_eq!(mock.count_metric("opendal_operation_errors_total"), 0);
        assert_eq!(mock.get_value_u64("opendal_operation_bytes"), Some(5));
        assert_eq!(mock.gauge_value("opendal_operation_executing"), 0);
    }

    #[test]
    fn test_list_wrapper_drop_records_operation_entries() {
        let mock = MockInterceptor::default();
        let labels = MetricLabels::new(test_info(), Operation::List.into_static());
        mock.observe(labels.clone(), MetricValue::OperationExecuting(1));

        let mut wrapper = MetricsWrapper {
            inner: (),
            interceptor: mock.clone(),
            labels,
            start: Instant::now(),
            size: 0,
            completed: true,
        };
        wrapper.size = 42;
        drop(wrapper);

        assert_eq!(mock.get_value_u64("opendal_operation_entries"), Some(42));
        assert!(mock.has_metric("opendal_operation_entries_rate"));
        assert_eq!(mock.get_value_u64("opendal_operation_bytes"), None);
        assert_eq!(mock.gauge_value("opendal_operation_executing"), 0);
    }

    #[test]
    fn test_wrapper_cancelled_records_operation_error() {
        let mock = MockInterceptor::default();
        let labels = MetricLabels::new(test_info(), Operation::Read.into_static());
        mock.observe(labels.clone(), MetricValue::OperationExecuting(1));

        let wrapper = MetricsWrapper {
            inner: (),
            interceptor: mock.clone(),
            labels,
            start: Instant::now(),
            size: 0,
            completed: false,
        };
        drop(wrapper);

        assert_eq!(mock.count_metric("opendal_operation_errors_total"), 1);
        assert_eq!(mock.gauge_value("opendal_operation_executing"), 0);
    }
}
