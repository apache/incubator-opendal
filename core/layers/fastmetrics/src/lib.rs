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
use fastmetrics::encoder::EncodeLabelSet;
use fastmetrics::encoder::LabelSetEncoder;
use fastmetrics::metrics::counter::Counter;
use fastmetrics::metrics::family::Family;
use fastmetrics::metrics::gauge::Gauge;
use fastmetrics::metrics::histogram::Histogram;
use fastmetrics::raw::LabelSetSchema;
use fastmetrics::registry::Register;
use fastmetrics::registry::Registry;
use fastmetrics::registry::with_global_registry_mut;
use opendal_core::raw::*;
use opendal_core::*;
use opendal_layer_observe_metrics_common as observe;

/// `FastmetricsLayer` records OpenDAL operation and HTTP fetch metrics with
/// [fastmetrics](https://docs.rs/fastmetrics/).
///
/// # Fastmetrics Metrics
///
/// This layer registers operation metrics and HTTP fetch metrics. Please see the documentation of
/// the [`observe`] module for metric details.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```no_run
/// # use fastmetrics::format::text::encode;
/// # use fastmetrics::format::text::TextProfile;
/// # use log::info;
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_fastmetrics::FastmetricsLayer;
/// #
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let mut registry = fastmetrics::registry::Registry::default();
/// let op = Operator::new(services::Memory::default())?
///     .layer(FastmetricsLayer::builder().register(&mut registry)?);
///
/// // Write data into object test.
/// op.write("test", "Hello, World!").await?;
///
/// // Read data from the object.
/// let bs = op.read("test").await?;
/// info!("content: {}", String::from_utf8_lossy(&bs.to_bytes()));
///
/// // Get object metadata.
/// let meta = op.stat("test").await?;
/// info!("meta: {:?}", meta);
///
/// // Export prometheus metrics.
/// let mut output = String::new();
/// encode(&mut output, &registry, TextProfile::PrometheusV0_0_4).unwrap();
/// println!("{}", output);
/// # Ok(())
/// # }
/// ```
/// ## Global Instance
///
/// `FastmetricsLayer` needs to be registered before instantiation.
///
/// If there are multiple operators in an application that need the `FastmetricsLayer`, we could
/// instantiate it once and pass it to multiple operators. But we cannot directly call
/// `.layer(FastmetricsLayer::builder().register(&mut registry)?)` for different services, because
/// registering the same metrics multiple times to the same registry will cause register errors.
/// Therefore, we can provide a global instance for the `FastmetricsLayer`.
///
/// ```no_run
/// # use std::sync::OnceLock;
/// #
/// # use fastmetrics::format::text::encode;
/// # use fastmetrics::format::text::TextProfile;
/// # use fastmetrics::registry::with_global_registry;
/// # use log::info;
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_fastmetrics::FastmetricsLayer;
/// #
/// fn global_fastmetrics_layer() -> &'static FastmetricsLayer {
///     static GLOBAL: OnceLock<FastmetricsLayer> = OnceLock::new();
///     GLOBAL.get_or_init(|| {
///         FastmetricsLayer::builder()
///             .register_global()
///             .expect("Failed to register with the global registry")
///     })
/// }
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// let op = Operator::new(services::Memory::default())?
///     .layer(global_fastmetrics_layer().clone());
///
/// // Write data into object test.
/// op.write("test", "Hello, World!").await?;
///
/// // Read data from the object.
/// let bs = op.read("test").await?;
/// info!("content: {}", String::from_utf8_lossy(&bs.to_bytes()));
///
/// // Get object metadata.
/// let meta = op.stat("test").await?;
/// info!("meta: {:?}", meta);
///
/// // Export prometheus metrics.
/// let mut output = String::new();
/// with_global_registry(|reg| encode(&mut output, &reg, TextProfile::PrometheusV0_0_4).unwrap());
/// println!("{}", output);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct FastmetricsLayer {
    interceptor: FastmetricsInterceptor,
}

impl FastmetricsLayer {
    /// Create a [`FastmetricsLayerBuilder`] to set the configuration of metrics.
    pub fn builder() -> FastmetricsLayerBuilder {
        FastmetricsLayerBuilder::default()
    }
}

impl Layer for FastmetricsLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        observe::MetricsLayer::new(self.interceptor.clone()).apply_service(inner)
    }

    // Reuse the same interceptor because this layer registers both operation and HTTP metrics.
    fn apply_context(&self, srv: Servicer, inner: OperationContext) -> OperationContext {
        observe::MetricsLayer::new(self.interceptor.clone()).apply_context(srv, inner)
    }
}

/// [`FastmetricsLayerBuilder`] is a config builder to build a [`FastmetricsLayer`].
pub struct FastmetricsLayerBuilder {
    bytes_buckets: Vec<f64>,
    bytes_rate_buckets: Vec<f64>,
    entries_buckets: Vec<f64>,
    entries_rate_buckets: Vec<f64>,
    duration_seconds_buckets: Vec<f64>,
    ttfb_buckets: Vec<f64>,
    disable_label_root: bool,
}

impl Default for FastmetricsLayerBuilder {
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

impl FastmetricsLayerBuilder {
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

    /// The 'root' label might have risks of being high cardinality; users can choose to disable it
    /// when they found it's not useful for their metrics.
    pub fn disable_label_root(mut self, disable: bool) -> Self {
        self.disable_label_root = disable;
        self
    }

    /// Register the metrics into the registry and return a [`FastmetricsLayer`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use opendal_core::services;
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # use opendal_layer_fastmetrics::FastmetricsLayer;
    /// #
    /// # fn main() -> Result<()> {
    /// let mut registry = fastmetrics::registry::Registry::default();
    ///
    /// // Pick a builder and configure it.
    /// let builder = services::Memory::default();
    /// let _ = Operator::new(builder)?
    ///     .layer(FastmetricsLayer::builder().register(&mut registry)?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn register(self, registry: &mut Registry) -> Result<FastmetricsLayer> {
        let Self {
            bytes_buckets,
            bytes_rate_buckets,
            entries_buckets,
            entries_rate_buckets,
            duration_seconds_buckets,
            ttfb_buckets,
            disable_label_root,
        } = self;

        let new_hist_family = |buckets: Vec<f64>| -> Family<OperationLabels, Histogram> {
            Family::new(move || Histogram::new(buckets.iter().copied()))
        };

        let operation_bytes = new_hist_family(bytes_buckets.clone());
        let operation_bytes_rate = new_hist_family(bytes_rate_buckets.clone());
        let operation_entries = new_hist_family(entries_buckets);
        let operation_entries_rate = new_hist_family(entries_rate_buckets);
        let operation_duration_seconds = new_hist_family(duration_seconds_buckets.clone());
        let operation_errors_total = Family::default();
        let operation_executing = Family::default();
        let operation_ttfb_seconds = new_hist_family(ttfb_buckets);

        let http_executing = Family::default();
        let http_request_bytes = new_hist_family(bytes_buckets.clone());
        let http_request_bytes_rate = new_hist_family(bytes_rate_buckets.clone());
        let http_request_duration_seconds = new_hist_family(duration_seconds_buckets.clone());
        let http_response_bytes = new_hist_family(bytes_buckets);
        let http_response_bytes_rate = new_hist_family(bytes_rate_buckets);
        let http_response_duration_seconds = new_hist_family(duration_seconds_buckets);
        let http_connection_errors_total = Family::default();
        let http_status_errors_total = Family::default();

        let interceptor = FastmetricsInterceptor {
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

            disable_label_root,
        };
        interceptor
            .register(registry)
            .map_err(|err| Error::new(ErrorKind::Unexpected, err.to_string()).set_source(err))?;

        Ok(FastmetricsLayer { interceptor })
    }

    /// Register the metrics into the global registry and return a [`FastmetricsLayer`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use opendal_core::services;
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # use opendal_layer_fastmetrics::FastmetricsLayer;
    /// #
    /// # fn main() -> Result<()> {
    /// // Pick a builder and configure it.
    /// let builder = services::Memory::default();
    /// let _ = Operator::new(builder)?
    ///     .layer(FastmetricsLayer::builder().register_global()?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn register_global(self) -> Result<FastmetricsLayer> {
        with_global_registry_mut(|registry| self.register(registry))
    }
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct FastmetricsInterceptor {
    operation_bytes: Family<OperationLabels, Histogram>,
    operation_bytes_rate: Family<OperationLabels, Histogram>,
    operation_entries: Family<OperationLabels, Histogram>,
    operation_entries_rate: Family<OperationLabels, Histogram>,
    operation_duration_seconds: Family<OperationLabels, Histogram>,
    operation_errors_total: Family<OperationLabels, Counter>,
    operation_executing: Family<OperationLabels, Gauge>,
    operation_ttfb_seconds: Family<OperationLabels, Histogram>,

    http_executing: Family<OperationLabels, Gauge>,
    http_request_bytes: Family<OperationLabels, Histogram>,
    http_request_bytes_rate: Family<OperationLabels, Histogram>,
    http_request_duration_seconds: Family<OperationLabels, Histogram>,
    http_response_bytes: Family<OperationLabels, Histogram>,
    http_response_bytes_rate: Family<OperationLabels, Histogram>,
    http_response_duration_seconds: Family<OperationLabels, Histogram>,
    http_connection_errors_total: Family<OperationLabels, Counter>,
    http_status_errors_total: Family<OperationLabels, Counter>,

    disable_label_root: bool,
}

impl Register for FastmetricsInterceptor {
    fn register(&self, registry: &mut Registry) -> fastmetrics::error::Result<()> {
        macro_rules! register_metrics {
            ($($field:ident => $value:expr),* $(,)?) => {
                $(
                    {
                        let ((name, unit), help) = ($value.name_with_unit(), $value.help());
                        registry.register_metric(name, help, unit, self.$field.clone())?;
                    }
                )*
            };
        }

        register_metrics! {
            // Operation metrics
            operation_bytes => observe::MetricValue::OperationBytes(0),
            operation_bytes_rate => observe::MetricValue::OperationBytesRate(0.0),
            operation_entries => observe::MetricValue::OperationEntries(0),
            operation_entries_rate => observe::MetricValue::OperationEntriesRate(0.0),
            operation_duration_seconds => observe::MetricValue::OperationDurationSeconds(Duration::default()),
            operation_errors_total => observe::MetricValue::OperationErrorsTotal,
            operation_executing => observe::MetricValue::OperationExecuting(0),
            operation_ttfb_seconds => observe::MetricValue::OperationTtfbSeconds(Duration::default()),

            // HTTP metrics
            http_executing => observe::MetricValue::HttpExecuting(0),
            http_request_bytes => observe::MetricValue::HttpRequestBytes(0),
            http_request_bytes_rate => observe::MetricValue::HttpRequestBytesRate(0.0),
            http_request_duration_seconds => observe::MetricValue::HttpRequestDurationSeconds(Duration::default()),
            http_response_bytes => observe::MetricValue::HttpResponseBytes(0),
            http_response_bytes_rate => observe::MetricValue::HttpResponseBytesRate(0.0),
            http_response_duration_seconds => observe::MetricValue::HttpResponseDurationSeconds(Duration::default()),
            http_connection_errors_total => observe::MetricValue::HttpConnectionErrorsTotal,
            http_status_errors_total => observe::MetricValue::HttpStatusErrorsTotal,
        }

        Ok(())
    }
}

impl observe::MetricsIntercept for FastmetricsInterceptor {
    fn observe(&self, labels: observe::MetricLabels, value: observe::MetricValue) {
        let labels = OperationLabels {
            labels,
            disable_label_root: self.disable_label_root,
        };
        match value {
            observe::MetricValue::OperationBytes(v) => {
                self.operation_bytes
                    .with_or_new(&labels, |hist| hist.observe(v as f64));
            }
            observe::MetricValue::OperationBytesRate(v) => {
                self.operation_bytes_rate
                    .with_or_new(&labels, |hist| hist.observe(v));
            }
            observe::MetricValue::OperationEntries(v) => {
                self.operation_entries
                    .with_or_new(&labels, |hist| hist.observe(v as f64));
            }
            observe::MetricValue::OperationEntriesRate(v) => {
                self.operation_entries_rate
                    .with_or_new(&labels, |hist| hist.observe(v));
            }
            observe::MetricValue::OperationDurationSeconds(v) => {
                self.operation_duration_seconds
                    .with_or_new(&labels, |hist| hist.observe(v.as_secs_f64()));
            }
            observe::MetricValue::OperationErrorsTotal => {
                self.operation_errors_total
                    .with_or_new(&labels, |counter| counter.inc());
            }
            observe::MetricValue::OperationExecuting(v) => {
                self.operation_executing
                    .with_or_new(&labels, |gauge| gauge.inc_by(v as i64));
            }
            observe::MetricValue::OperationTtfbSeconds(v) => {
                self.operation_ttfb_seconds
                    .with_or_new(&labels, |hist| hist.observe(v.as_secs_f64()));
            }

            observe::MetricValue::HttpExecuting(v) => {
                self.http_executing
                    .with_or_new(&labels, |gauge| gauge.inc_by(v as i64));
            }
            observe::MetricValue::HttpRequestBytes(v) => {
                self.http_request_bytes
                    .with_or_new(&labels, |hist| hist.observe(v as f64));
            }
            observe::MetricValue::HttpRequestBytesRate(v) => {
                self.http_request_bytes_rate
                    .with_or_new(&labels, |hist| hist.observe(v));
            }
            observe::MetricValue::HttpRequestDurationSeconds(v) => {
                self.http_request_duration_seconds
                    .with_or_new(&labels, |hist| hist.observe(v.as_secs_f64()));
            }
            observe::MetricValue::HttpResponseBytes(v) => {
                self.http_response_bytes
                    .with_or_new(&labels, |hist| hist.observe(v as f64));
            }
            observe::MetricValue::HttpResponseBytesRate(v) => {
                self.http_response_bytes_rate
                    .with_or_new(&labels, |hist| hist.observe(v));
            }
            observe::MetricValue::HttpResponseDurationSeconds(v) => {
                self.http_response_duration_seconds
                    .with_or_new(&labels, |hist| hist.observe(v.as_secs_f64()));
            }
            observe::MetricValue::HttpConnectionErrorsTotal => {
                self.http_connection_errors_total
                    .with_or_new(&labels, |counter| counter.inc());
            }
            observe::MetricValue::HttpStatusErrorsTotal => {
                self.http_status_errors_total
                    .with_or_new(&labels, |counter| counter.inc());
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

impl LabelSetSchema for OperationLabels {
    fn names() -> Option<&'static [&'static str]> {
        static NAMES: &[&str] = &[
            observe::LABEL_SCHEME,
            observe::LABEL_NAMESPACE,
            observe::LABEL_ROOT,
            observe::LABEL_OPERATION,
            observe::LABEL_ERROR,
            observe::LABEL_STATUS_CODE,
            observe::LABEL_SERVICE_OPERATION,
        ];
        Some(NAMES)
    }
}

impl EncodeLabelSet for OperationLabels {
    fn encode(&self, encoder: &mut dyn LabelSetEncoder) -> fastmetrics::error::Result<()> {
        encoder.encode(&(observe::LABEL_SCHEME, self.labels.scheme))?;
        encoder.encode(&(observe::LABEL_NAMESPACE, self.labels.namespace.as_ref()))?;
        if !self.disable_label_root {
            encoder.encode(&(observe::LABEL_ROOT, self.labels.root.as_ref()))?;
        }
        encoder.encode(&(observe::LABEL_OPERATION, self.labels.operation))?;
        if let Some(error) = &self.labels.error {
            encoder.encode(&(observe::LABEL_ERROR, error.into_static()))?;
        }
        if let Some(code) = &self.labels.status_code {
            encoder.encode(&(observe::LABEL_STATUS_CODE, code.as_str()))?;
        }
        if let Some(service_operation) = self.labels.service_operation {
            encoder.encode(&(observe::LABEL_SERVICE_OPERATION, service_operation))?;
        }
        Ok(())
    }
}
