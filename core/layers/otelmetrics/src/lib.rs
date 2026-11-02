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
use opendal_core::OperationContext;
use opendal_core::raw::*;
use opendal_layer_observe_metrics_common as observe;
use opentelemetry::KeyValue;
use opentelemetry::metrics::Counter;
use opentelemetry::metrics::Histogram;
use opentelemetry::metrics::Meter;
use opentelemetry::metrics::UpDownCounter;

/// `OtelMetricsLayer` records OpenDAL operation and HTTP fetch metrics with
/// [opentelemetry::metrics].
///
/// This layer records operation metrics from OpenDAL API calls and HTTP metrics
/// from requests made through OpenDAL's HTTP fetcher.
///
/// # Examples
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_otelmetrics::OtelMetricsLayer;
/// #
/// # fn main() -> Result<()> {
/// let meter = opentelemetry::global::meter("opendal");
/// let _ = Operator::new(services::Memory::default())?
///     .layer(OtelMetricsLayer::builder().register(&meter));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct OtelMetricsLayer {
    interceptor: OtelMetricsInterceptor,
}

impl OtelMetricsLayer {
    /// Create a [`OtelMetricsLayerBuilder`] to set the configuration of metrics.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal_core::services;
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # use opendal_layer_otelmetrics::OtelMetricsLayer;
    /// #
    /// # fn main() -> Result<()> {
    /// let meter = opentelemetry::global::meter("opendal");
    /// let op = Operator::new(services::Memory::default())?
    ///     .layer(OtelMetricsLayer::builder().register(&meter));
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> OtelMetricsLayerBuilder {
        OtelMetricsLayerBuilder::default()
    }
}

/// [`OtelMetricsLayerBuilder`] is a config builder to build a [`OtelMetricsLayer`].
pub struct OtelMetricsLayerBuilder {
    bytes_boundaries: Vec<f64>,
    bytes_rate_boundaries: Vec<f64>,
    entries_boundaries: Vec<f64>,
    entries_rate_boundaries: Vec<f64>,
    duration_seconds_boundaries: Vec<f64>,
    ttfb_boundaries: Vec<f64>,
}

impl Default for OtelMetricsLayerBuilder {
    fn default() -> Self {
        Self {
            bytes_boundaries: observe::DEFAULT_BYTES_BUCKETS.to_vec(),
            bytes_rate_boundaries: observe::DEFAULT_BYTES_RATE_BUCKETS.to_vec(),
            entries_boundaries: observe::DEFAULT_ENTRIES_BUCKETS.to_vec(),
            entries_rate_boundaries: observe::DEFAULT_ENTRIES_RATE_BUCKETS.to_vec(),
            duration_seconds_boundaries: observe::DEFAULT_DURATION_SECONDS_BUCKETS.to_vec(),
            ttfb_boundaries: observe::DEFAULT_TTFB_BUCKETS.to_vec(),
        }
    }
}

impl OtelMetricsLayerBuilder {
    /// Set boundaries for bytes histograms, including operation bytes and HTTP body sizes.
    pub fn bytes_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        if !boundaries.is_empty() {
            self.bytes_boundaries = boundaries;
        }
        self
    }

    /// Set boundaries for bytes rate histograms, including operation and HTTP body rates.
    pub fn bytes_rate_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        if !boundaries.is_empty() {
            self.bytes_rate_boundaries = boundaries;
        }
        self
    }

    /// Set boundaries for entries related histogram like `operation_entries`.
    pub fn entries_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        if !boundaries.is_empty() {
            self.entries_boundaries = boundaries;
        }
        self
    }

    /// Set boundaries for entries rate related histogram like `operation_entries_rate`.
    pub fn entries_rate_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        if !boundaries.is_empty() {
            self.entries_rate_boundaries = boundaries;
        }
        self
    }

    /// Set boundaries for duration histograms, including operation and HTTP request/response durations.
    pub fn duration_seconds_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        if !boundaries.is_empty() {
            self.duration_seconds_boundaries = boundaries;
        }
        self
    }

    /// Set boundaries for ttfb related histogram like `operation_ttfb_seconds`.
    pub fn ttfb_boundaries(mut self, boundaries: Vec<f64>) -> Self {
        if !boundaries.is_empty() {
            self.ttfb_boundaries = boundaries;
        }
        self
    }

    /// Register the metrics and return a [`OtelMetricsLayer`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal_core::services;
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # use opendal_layer_otelmetrics::OtelMetricsLayer;
    /// #
    /// # fn main() -> Result<()> {
    /// let meter = opentelemetry::global::meter("opendal");
    /// let op = Operator::new(services::Memory::default())?
    ///     .layer(OtelMetricsLayer::builder().register(&meter));
    /// # Ok(())
    /// # }
    /// ```
    pub fn register(self, meter: &Meter) -> OtelMetricsLayer {
        let operation_bytes = {
            let metric = observe::MetricValue::OperationBytes(0);
            register_u64_histogram_meter(
                meter,
                "opendal.operation.bytes",
                metric,
                self.bytes_boundaries.clone(),
            )
        };
        let operation_bytes_rate = {
            let metric = observe::MetricValue::OperationBytesRate(0.0);
            register_f64_histogram_meter(
                meter,
                "opendal.operation.bytes_rate",
                metric,
                self.bytes_rate_boundaries.clone(),
            )
        };
        let operation_entries = {
            let metric = observe::MetricValue::OperationEntries(0);
            register_u64_histogram_meter(
                meter,
                "opendal.operation.entries",
                metric,
                self.entries_boundaries.clone(),
            )
        };
        let operation_entries_rate = {
            let metric = observe::MetricValue::OperationEntriesRate(0.0);
            register_f64_histogram_meter(
                meter,
                "opendal.operation.entries_rate",
                metric,
                self.entries_rate_boundaries.clone(),
            )
        };
        let operation_duration_seconds = {
            let metric = observe::MetricValue::OperationDurationSeconds(Duration::default());
            register_f64_histogram_meter(
                meter,
                "opendal.operation.duration",
                metric,
                self.duration_seconds_boundaries.clone(),
            )
        };
        let operation_errors_total = {
            let metric = observe::MetricValue::OperationErrorsTotal;
            meter
                .u64_counter("opendal.operation.errors")
                .with_description(metric.help())
                .build()
        };
        let operation_executing = {
            let metric = observe::MetricValue::OperationExecuting(0);
            meter
                .i64_up_down_counter("opendal.operation.executing")
                .with_description(metric.help())
                .build()
        };
        let operation_ttfb_seconds = {
            let metric = observe::MetricValue::OperationTtfbSeconds(Duration::default());
            register_f64_histogram_meter(
                meter,
                "opendal.operation.ttfb",
                metric,
                self.ttfb_boundaries.clone(),
            )
        };

        let http_executing = {
            let metric = observe::MetricValue::HttpExecuting(0);
            meter
                .i64_up_down_counter("opendal.http.executing")
                .with_description(metric.help())
                .build()
        };
        let http_request_bytes = {
            let metric = observe::MetricValue::HttpRequestBytes(0);
            register_u64_histogram_meter(
                meter,
                "opendal.http.request.bytes",
                metric,
                self.bytes_boundaries.clone(),
            )
        };
        let http_request_bytes_rate = {
            let metric = observe::MetricValue::HttpRequestBytesRate(0.0);
            register_f64_histogram_meter(
                meter,
                "opendal.http.request.bytes_rate",
                metric,
                self.bytes_rate_boundaries.clone(),
            )
        };
        let http_request_duration_seconds = {
            let metric = observe::MetricValue::HttpRequestDurationSeconds(Duration::default());
            register_f64_histogram_meter(
                meter,
                "opendal.http.request.duration",
                metric,
                self.duration_seconds_boundaries.clone(),
            )
        };
        let http_response_bytes = {
            let metric = observe::MetricValue::HttpResponseBytes(0);
            register_u64_histogram_meter(
                meter,
                "opendal.http.response.bytes",
                metric,
                self.bytes_boundaries.clone(),
            )
        };
        let http_response_bytes_rate = {
            let metric = observe::MetricValue::HttpResponseBytesRate(0.0);
            register_f64_histogram_meter(
                meter,
                "opendal.http.response.bytes_rate",
                metric,
                self.bytes_rate_boundaries.clone(),
            )
        };
        let http_response_duration_seconds = {
            let metric = observe::MetricValue::HttpResponseDurationSeconds(Duration::default());
            register_f64_histogram_meter(
                meter,
                "opendal.http.response.duration",
                metric,
                self.duration_seconds_boundaries.clone(),
            )
        };
        let http_connection_errors_total = {
            let metric = observe::MetricValue::HttpConnectionErrorsTotal;
            meter
                .u64_counter("opendal.http.connection_errors")
                .with_description(metric.help())
                .build()
        };
        let http_status_errors_total = {
            let metric = observe::MetricValue::HttpStatusErrorsTotal;
            meter
                .u64_counter("opendal.http.status_errors")
                .with_description(metric.help())
                .build()
        };

        OtelMetricsLayer {
            interceptor: OtelMetricsInterceptor {
                operation_bytes,
                operation_bytes_rate,
                operation_entries,
                operation_entries_rate,
                operation_duration_seconds,
                operation_errors_total,
                operation_executing,
                operation_ttfb_seconds,

                http_executing,
                http_request_bytes,
                http_request_bytes_rate,
                http_request_duration_seconds,
                http_response_bytes,
                http_response_bytes_rate,
                http_response_duration_seconds,
                http_connection_errors_total,
                http_status_errors_total,
            },
        }
    }
}

impl Layer for OtelMetricsLayer {
    // Operation and HTTP metrics share the same registered OpenTelemetry instruments.
    fn apply_service(&self, inner: Servicer) -> Servicer {
        observe::MetricsLayer::new(self.interceptor.clone()).apply_service(inner)
    }

    fn apply_context(&self, srv: Servicer, inner: OperationContext) -> OperationContext {
        observe::MetricsLayer::new(self.interceptor.clone()).apply_context(srv, inner)
    }
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct OtelMetricsInterceptor {
    operation_bytes: Histogram<u64>,
    operation_bytes_rate: Histogram<f64>,
    operation_entries: Histogram<u64>,
    operation_entries_rate: Histogram<f64>,
    operation_duration_seconds: Histogram<f64>,
    operation_errors_total: Counter<u64>,
    operation_executing: UpDownCounter<i64>,
    operation_ttfb_seconds: Histogram<f64>,

    http_executing: UpDownCounter<i64>,
    http_request_bytes: Histogram<u64>,
    http_request_bytes_rate: Histogram<f64>,
    http_request_duration_seconds: Histogram<f64>,
    http_response_bytes: Histogram<u64>,
    http_response_bytes_rate: Histogram<f64>,
    http_response_duration_seconds: Histogram<f64>,
    http_connection_errors_total: Counter<u64>,
    http_status_errors_total: Counter<u64>,
}

impl observe::MetricsIntercept for OtelMetricsInterceptor {
    fn observe(&self, labels: observe::MetricLabels, value: observe::MetricValue) {
        let attributes = self.create_attributes(labels);

        match value {
            observe::MetricValue::OperationBytes(v) => self.operation_bytes.record(v, &attributes),
            observe::MetricValue::OperationBytesRate(v) => {
                self.operation_bytes_rate.record(v, &attributes)
            }
            observe::MetricValue::OperationEntries(v) => {
                self.operation_entries.record(v, &attributes)
            }
            observe::MetricValue::OperationEntriesRate(v) => {
                self.operation_entries_rate.record(v, &attributes)
            }
            observe::MetricValue::OperationDurationSeconds(v) => self
                .operation_duration_seconds
                .record(v.as_secs_f64(), &attributes),
            observe::MetricValue::OperationErrorsTotal => {
                self.operation_errors_total.add(1, &attributes)
            }
            observe::MetricValue::OperationExecuting(v) => {
                self.operation_executing.add(v as i64, &attributes)
            }
            observe::MetricValue::OperationTtfbSeconds(v) => self
                .operation_ttfb_seconds
                .record(v.as_secs_f64(), &attributes),

            observe::MetricValue::HttpExecuting(v) => {
                self.http_executing.add(v as i64, &attributes)
            }
            observe::MetricValue::HttpRequestBytes(v) => {
                self.http_request_bytes.record(v, &attributes)
            }
            observe::MetricValue::HttpRequestBytesRate(v) => {
                self.http_request_bytes_rate.record(v, &attributes)
            }
            observe::MetricValue::HttpRequestDurationSeconds(v) => self
                .http_request_duration_seconds
                .record(v.as_secs_f64(), &attributes),
            observe::MetricValue::HttpResponseBytes(v) => {
                self.http_response_bytes.record(v, &attributes)
            }
            observe::MetricValue::HttpResponseBytesRate(v) => {
                self.http_response_bytes_rate.record(v, &attributes)
            }
            observe::MetricValue::HttpResponseDurationSeconds(v) => self
                .http_response_duration_seconds
                .record(v.as_secs_f64(), &attributes),
            observe::MetricValue::HttpConnectionErrorsTotal => {
                self.http_connection_errors_total.add(1, &attributes)
            }
            observe::MetricValue::HttpStatusErrorsTotal => {
                self.http_status_errors_total.add(1, &attributes)
            }
            _ => {}
        }
    }
}

impl OtelMetricsInterceptor {
    fn create_attributes(&self, attrs: observe::MetricLabels) -> Vec<KeyValue> {
        let mut attributes = Vec::with_capacity(6);

        attributes.extend([
            KeyValue::new(observe::LABEL_SCHEME, attrs.scheme),
            KeyValue::new(observe::LABEL_NAMESPACE, attrs.namespace),
            KeyValue::new(observe::LABEL_ROOT, attrs.root),
            KeyValue::new(observe::LABEL_OPERATION, attrs.operation),
        ]);

        if let Some(error) = attrs.error {
            attributes.push(KeyValue::new(observe::LABEL_ERROR, error.into_static()));
        }

        if let Some(status_code) = attrs.status_code {
            attributes.push(KeyValue::new(
                observe::LABEL_STATUS_CODE,
                status_code.as_u16() as i64,
            ));
        }

        if let Some(service_operation) = attrs.service_operation {
            attributes.push(KeyValue::new(
                observe::LABEL_SERVICE_OPERATION,
                service_operation,
            ));
        }

        attributes
    }
}

fn register_u64_histogram_meter(
    meter: &Meter,
    name: &'static str,
    metric: observe::MetricValue,
    boundaries: Vec<f64>,
) -> Histogram<u64> {
    let (_name, unit) = metric.name_with_unit();
    let description = metric.help();

    let builder = meter
        .u64_histogram(name)
        .with_description(description)
        .with_boundaries(boundaries);

    if let Some(unit) = unit {
        builder.with_unit(unit).build()
    } else {
        builder.build()
    }
}

fn register_f64_histogram_meter(
    meter: &Meter,
    name: &'static str,
    metric: observe::MetricValue,
    boundaries: Vec<f64>,
) -> Histogram<f64> {
    let (_name, unit) = metric.name_with_unit();
    let description = metric.help();

    let builder = meter
        .f64_histogram(name)
        .with_description(description)
        .with_boundaries(boundaries);

    if let Some(unit) = unit {
        builder.with_unit(unit).build()
    } else {
        builder.build()
    }
}
