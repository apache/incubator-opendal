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
use prometheus_client::encoding::EncodeLabel;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::encoding::LabelSetEncoder;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::family::MetricConstructor;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Metric;
use prometheus_client::registry::Registry;
use prometheus_client::registry::Unit;

/// `PrometheusClientLayer` records OpenDAL operation and HTTP fetch metrics with
/// [prometheus-client](https://docs.rs/prometheus-client).
///
/// # Prometheus Metrics
///
/// This layer registers operation metrics and HTTP fetch metrics. Please see the documentation of [`observe`] module for metric details.
/// For a more detailed explanation of these metrics and how they are used, please refer to the [Prometheus documentation](https://prometheus.io/docs/introduction/overview/).
///
/// # Examples
///
/// ```no_run
/// # use log::info;
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_prometheus_client::PrometheusClientLayer;
/// #
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let mut registry = prometheus_client::registry::Registry::default();
///
/// let op = Operator::new(services::Memory::default())?
///     .layer(PrometheusClientLayer::builder().register(&mut registry));
///
/// // Write data into object test.
/// op.write("test", "Hello, World!").await?;
/// // Read data from the object.
/// let bs = op.read("test").await?;
/// info!("content: {}", String::from_utf8_lossy(&bs.to_bytes()));
///
/// // Get object metadata.
/// let meta = op.stat("test").await?;
/// info!("meta: {:?}", meta);
///
/// // Export prometheus metrics.
/// let mut buf = String::new();
/// prometheus_client::encoding::text::encode(&mut buf, &registry).unwrap();
/// println!("## Prometheus Metrics");
/// println!("{}", buf);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct PrometheusClientLayer {
    interceptor: PrometheusClientInterceptor,
}

impl PrometheusClientLayer {
    /// Create a [`PrometheusClientLayerBuilder`] to set the configuration of metrics.
    pub fn builder() -> PrometheusClientLayerBuilder {
        PrometheusClientLayerBuilder::default()
    }
}

impl Layer for PrometheusClientLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        observe::MetricsLayer::new(self.interceptor.clone()).apply_service(inner)
    }

    // Reuse the same interceptor because this layer registers both operation and HTTP metrics.
    fn apply_context(&self, srv: Servicer, inner: OperationContext) -> OperationContext {
        observe::MetricsLayer::new(self.interceptor.clone()).apply_context(srv, inner)
    }
}

/// [`PrometheusClientLayerBuilder`] is a config builder to build a [`PrometheusClientLayer`].
pub struct PrometheusClientLayerBuilder {
    bytes_buckets: Vec<f64>,
    bytes_rate_buckets: Vec<f64>,
    entries_buckets: Vec<f64>,
    entries_rate_buckets: Vec<f64>,
    duration_seconds_buckets: Vec<f64>,
    ttfb_buckets: Vec<f64>,
    disable_label_root: bool,
}

impl Default for PrometheusClientLayerBuilder {
    fn default() -> Self {
        Self {
            bytes_buckets: observe::DEFAULT_BYTES_BUCKETS.to_vec(),
            bytes_rate_buckets: observe::DEFAULT_BYTES_RATE_BUCKETS.to_vec(),
            entries_buckets: observe::DEFAULT_ENTRIES_BUCKETS.to_vec(),
            entries_rate_buckets: observe::DEFAULT_ENTRIES_RATE_BUCKETS.to_vec(),
            duration_seconds_buckets: observe::DEFAULT_DURATION_SECONDS_BUCKETS.to_vec(),
            ttfb_buckets: observe::DEFAULT_TTFB_BUCKETS.to_vec(),
            disable_label_root: false,
        }
    }
}

impl PrometheusClientLayerBuilder {
    /// Set buckets for bytes related histogram like `operation_bytes`.
    pub fn bytes_buckets(mut self, buckets: Vec<f64>) -> Self {
        if !buckets.is_empty() {
            self.bytes_buckets = buckets;
        }
        self
    }

    /// Set buckets for bytes rate related histogram like `operation_bytes_rate`.
    pub fn bytes_rate_buckets(mut self, buckets: Vec<f64>) -> Self {
        if !buckets.is_empty() {
            self.bytes_rate_buckets = buckets;
        }
        self
    }

    /// Set buckets for entries related histogram like `operation_entries`.
    pub fn entries_buckets(mut self, buckets: Vec<f64>) -> Self {
        if !buckets.is_empty() {
            self.entries_buckets = buckets;
        }
        self
    }

    /// Set buckets for entries rate related histogram like `operation_entries_rate`.
    pub fn entries_rate_buckets(mut self, buckets: Vec<f64>) -> Self {
        if !buckets.is_empty() {
            self.entries_rate_buckets = buckets;
        }
        self
    }

    /// Set buckets for duration seconds related histogram like `operation_duration_seconds`.
    pub fn duration_seconds_buckets(mut self, buckets: Vec<f64>) -> Self {
        if !buckets.is_empty() {
            self.duration_seconds_buckets = buckets;
        }
        self
    }

    /// Set buckets for ttfb related histogram like `operation_ttfb_seconds`.
    pub fn ttfb_buckets(mut self, buckets: Vec<f64>) -> Self {
        if !buckets.is_empty() {
            self.ttfb_buckets = buckets;
        }
        self
    }

    /// The 'root' label might have risks of being high cardinality, users can choose to disable it
    /// when they found it's not useful for their metrics.
    pub fn disable_label_root(mut self, disable: bool) -> Self {
        self.disable_label_root = disable;
        self
    }

    /// Register the metrics into the registry and return a [`PrometheusClientLayer`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use opendal_core::services;
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # use opendal_layer_prometheus_client::PrometheusClientLayer;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// // Pick a builder and configure it.
    /// let builder = services::Memory::default();
    /// let mut registry = prometheus_client::registry::Registry::default();
    ///
    /// let _ = Operator::new(builder)?
    ///     .layer(PrometheusClientLayer::builder().register(&mut registry));
    /// # Ok(())
    /// # }
    /// ```
    pub fn register(self, registry: &mut Registry) -> PrometheusClientLayer {
        let operation_bytes =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.bytes_buckets.clone(),
            });
        let operation_bytes_rate =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.bytes_rate_buckets.clone(),
            });
        let operation_entries =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.entries_buckets.clone(),
            });
        let operation_entries_rate =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.entries_rate_buckets.clone(),
            });
        let operation_duration_seconds =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.duration_seconds_buckets.clone(),
            });
        let operation_errors_total = Family::<OperationLabels, Counter>::default();
        let operation_executing = Family::<OperationLabels, Gauge>::default();
        let operation_ttfb_seconds =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.ttfb_buckets.clone(),
            });

        let http_executing = Family::<OperationLabels, Gauge>::default();
        let http_request_bytes =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.bytes_buckets.clone(),
            });
        let http_request_bytes_rate =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.bytes_rate_buckets.clone(),
            });
        let http_request_duration_seconds =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.duration_seconds_buckets.clone(),
            });
        let http_response_bytes =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.bytes_buckets.clone(),
            });
        let http_response_bytes_rate =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.bytes_rate_buckets.clone(),
            });
        let http_response_duration_seconds =
            Family::<OperationLabels, Histogram, _>::new_with_constructor(HistogramConstructor {
                buckets: self.duration_seconds_buckets.clone(),
            });
        let http_connection_errors_total = Family::<OperationLabels, Counter>::default();
        let http_status_errors_total = Family::<OperationLabels, Counter>::default();

        register_metric(
            registry,
            operation_bytes.clone(),
            observe::MetricValue::OperationBytes(0),
        );
        register_metric(
            registry,
            operation_bytes_rate.clone(),
            observe::MetricValue::OperationBytesRate(0.0),
        );
        register_metric(
            registry,
            operation_entries.clone(),
            observe::MetricValue::OperationEntries(0),
        );
        register_metric(
            registry,
            operation_entries_rate.clone(),
            observe::MetricValue::OperationEntriesRate(0.0),
        );
        register_metric(
            registry,
            operation_duration_seconds.clone(),
            observe::MetricValue::OperationDurationSeconds(Duration::default()),
        );
        register_metric(
            registry,
            operation_errors_total.clone(),
            observe::MetricValue::OperationErrorsTotal,
        );
        register_metric(
            registry,
            operation_executing.clone(),
            observe::MetricValue::OperationExecuting(0),
        );
        register_metric(
            registry,
            operation_ttfb_seconds.clone(),
            observe::MetricValue::OperationTtfbSeconds(Duration::default()),
        );

        register_metric(
            registry,
            http_executing.clone(),
            observe::MetricValue::HttpExecuting(0),
        );
        register_metric(
            registry,
            http_request_bytes.clone(),
            observe::MetricValue::HttpRequestBytes(0),
        );
        register_metric(
            registry,
            http_request_bytes_rate.clone(),
            observe::MetricValue::HttpRequestBytesRate(0.0),
        );
        register_metric(
            registry,
            http_request_duration_seconds.clone(),
            observe::MetricValue::HttpRequestDurationSeconds(Duration::default()),
        );
        register_metric(
            registry,
            http_response_bytes.clone(),
            observe::MetricValue::HttpResponseBytes(0),
        );
        register_metric(
            registry,
            http_response_bytes_rate.clone(),
            observe::MetricValue::HttpResponseBytesRate(0.0),
        );
        register_metric(
            registry,
            http_response_duration_seconds.clone(),
            observe::MetricValue::HttpResponseDurationSeconds(Duration::default()),
        );
        register_metric(
            registry,
            http_connection_errors_total.clone(),
            observe::MetricValue::HttpConnectionErrorsTotal,
        );
        register_metric(
            registry,
            http_status_errors_total.clone(),
            observe::MetricValue::HttpStatusErrorsTotal,
        );

        PrometheusClientLayer {
            interceptor: PrometheusClientInterceptor {
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

                disable_label_root: self.disable_label_root,
            },
        }
    }
}

#[derive(Clone)]
struct HistogramConstructor {
    buckets: Vec<f64>,
}

impl MetricConstructor<Histogram> for HistogramConstructor {
    fn new_metric(&self) -> Histogram {
        Histogram::new(self.buckets.iter().cloned())
    }
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct PrometheusClientInterceptor {
    operation_bytes: Family<OperationLabels, Histogram, HistogramConstructor>,
    operation_bytes_rate: Family<OperationLabels, Histogram, HistogramConstructor>,
    operation_entries: Family<OperationLabels, Histogram, HistogramConstructor>,
    operation_entries_rate: Family<OperationLabels, Histogram, HistogramConstructor>,
    operation_duration_seconds: Family<OperationLabels, Histogram, HistogramConstructor>,
    operation_errors_total: Family<OperationLabels, Counter>,
    operation_executing: Family<OperationLabels, Gauge>,
    operation_ttfb_seconds: Family<OperationLabels, Histogram, HistogramConstructor>,

    http_executing: Family<OperationLabels, Gauge>,
    http_request_bytes: Family<OperationLabels, Histogram, HistogramConstructor>,
    http_request_bytes_rate: Family<OperationLabels, Histogram, HistogramConstructor>,
    http_request_duration_seconds: Family<OperationLabels, Histogram, HistogramConstructor>,
    http_response_bytes: Family<OperationLabels, Histogram, HistogramConstructor>,
    http_response_bytes_rate: Family<OperationLabels, Histogram, HistogramConstructor>,
    http_response_duration_seconds: Family<OperationLabels, Histogram, HistogramConstructor>,
    http_connection_errors_total: Family<OperationLabels, Counter>,
    http_status_errors_total: Family<OperationLabels, Counter>,

    disable_label_root: bool,
}

impl observe::MetricsIntercept for PrometheusClientInterceptor {
    fn observe(&self, labels: observe::MetricLabels, value: observe::MetricValue) {
        let labels = OperationLabels {
            labels,
            disable_label_root: self.disable_label_root,
        };
        match value {
            observe::MetricValue::OperationBytes(v) => self
                .operation_bytes
                .get_or_create(&labels)
                .observe(v as f64),
            observe::MetricValue::OperationBytesRate(v) => {
                self.operation_bytes_rate.get_or_create(&labels).observe(v)
            }
            observe::MetricValue::OperationEntries(v) => self
                .operation_entries
                .get_or_create(&labels)
                .observe(v as f64),
            observe::MetricValue::OperationEntriesRate(v) => self
                .operation_entries_rate
                .get_or_create(&labels)
                .observe(v),
            observe::MetricValue::OperationDurationSeconds(v) => self
                .operation_duration_seconds
                .get_or_create(&labels)
                .observe(v.as_secs_f64()),
            observe::MetricValue::OperationErrorsTotal => {
                self.operation_errors_total.get_or_create(&labels).inc();
            }
            observe::MetricValue::OperationExecuting(v) => {
                self.operation_executing
                    .get_or_create(&labels)
                    .inc_by(v as i64);
            }
            observe::MetricValue::OperationTtfbSeconds(v) => self
                .operation_ttfb_seconds
                .get_or_create(&labels)
                .observe(v.as_secs_f64()),

            observe::MetricValue::HttpExecuting(v) => {
                self.http_executing.get_or_create(&labels).inc_by(v as i64);
            }
            observe::MetricValue::HttpRequestBytes(v) => self
                .http_request_bytes
                .get_or_create(&labels)
                .observe(v as f64),
            observe::MetricValue::HttpRequestBytesRate(v) => self
                .http_request_bytes_rate
                .get_or_create(&labels)
                .observe(v),
            observe::MetricValue::HttpRequestDurationSeconds(v) => self
                .http_request_duration_seconds
                .get_or_create(&labels)
                .observe(v.as_secs_f64()),
            observe::MetricValue::HttpResponseBytes(v) => self
                .http_response_bytes
                .get_or_create(&labels)
                .observe(v as f64),
            observe::MetricValue::HttpResponseBytesRate(v) => self
                .http_response_bytes_rate
                .get_or_create(&labels)
                .observe(v),
            observe::MetricValue::HttpResponseDurationSeconds(v) => self
                .http_response_duration_seconds
                .get_or_create(&labels)
                .observe(v.as_secs_f64()),
            observe::MetricValue::HttpConnectionErrorsTotal => {
                self.http_connection_errors_total
                    .get_or_create(&labels)
                    .inc();
            }
            observe::MetricValue::HttpStatusErrorsTotal => {
                self.http_status_errors_total.get_or_create(&labels).inc();
            }
            _ => {}
        };
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct OperationLabels {
    labels: observe::MetricLabels,
    disable_label_root: bool,
}

impl EncodeLabelSet for OperationLabels {
    fn encode(&self, encoder: &mut LabelSetEncoder<'_>) -> std::fmt::Result {
        (observe::LABEL_SCHEME, self.labels.scheme).encode(encoder.encode_label())?;
        (observe::LABEL_NAMESPACE, self.labels.namespace.as_ref())
            .encode(encoder.encode_label())?;
        if !self.disable_label_root {
            (observe::LABEL_ROOT, self.labels.root.as_ref()).encode(encoder.encode_label())?;
        }
        (observe::LABEL_OPERATION, self.labels.operation).encode(encoder.encode_label())?;

        if let Some(error) = &self.labels.error {
            (observe::LABEL_ERROR, error.into_static()).encode(encoder.encode_label())?;
        }
        if let Some(code) = &self.labels.status_code {
            (observe::LABEL_STATUS_CODE, code.as_str()).encode(encoder.encode_label())?;
        }
        if let Some(service_operation) = self.labels.service_operation {
            (observe::LABEL_SERVICE_OPERATION, service_operation).encode(encoder.encode_label())?;
        }
        Ok(())
    }
}

fn register_metric(registry: &mut Registry, metric: impl Metric, value: observe::MetricValue) {
    let ((name, unit), help) = (value.name_with_unit(), value.help());

    if let Some(unit) = unit {
        registry.register_with_unit(name, help, Unit::Other(unit.to_string()), metric);
    } else {
        registry.register(name, help, metric);
    }
}
