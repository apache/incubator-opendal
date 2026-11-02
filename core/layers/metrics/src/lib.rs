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
use metrics::Label;
use metrics::counter;
use metrics::gauge;
use metrics::histogram;
use opendal_core::OperationContext;
use opendal_core::raw::*;
use opendal_layer_observe_metrics_common as observe;

/// `MetricsLayer` records every operation with
/// [metrics](https://docs.rs/metrics/).
///
/// # Metrics
///
/// The [`observe`] module documents the metrics this layer records.
///
/// # Notes
///
/// Please make sure the exporter has been pulled in regular time.
/// Otherwise, the histogram data collected by `requests_duration_seconds`
/// could result in OOM.
///
/// # Examples
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_metrics::MetricsLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(MetricsLayer::default());
/// # Ok(())
/// # }
/// ```
///
/// # Output
///
/// OpenDAL uses [`metrics`](https://docs.rs/metrics/latest/metrics/) internally.
///
/// To enable metrics output, please enable one of the exporters that `metrics` supports.
///
/// Take [`metrics_exporter_prometheus`](https://docs.rs/metrics-exporter-prometheus/latest/metrics_exporter_prometheus/) as an example:
///
/// ```ignore
/// let builder = PrometheusBuilder::new()
///     .set_buckets_for_metric(
///         Matcher::Suffix("bytes".into()),
///         &observe::DEFAULT_BYTES_BUCKETS,
///     )
///     .set_buckets_for_metric(
///         Matcher::Suffix("duration_seconds".into()),
///         &observe::DEFAULT_DURATION_SECONDS_BUCKETS,
///     )
///     // ..
///     .expect("failed to create builder");
/// builder.install().expect("failed to install recorder/exporter");
/// let handle = builder.install_recorder().expect("failed to install recorder");
/// let (recorder, exporter) = builder.build().expect("failed to build recorder/exporter");
/// let recorder = builder.build_recorder().expect("failed to build recorder");
/// ```
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct MetricsLayer {}

impl MetricsLayer {
    /// Create a new [`MetricsLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for MetricsLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        let interceptor = MetricsInterceptor {};
        observe::MetricsLayer::new(interceptor).apply_service(inner)
    }

    fn apply_context(&self, srv: Servicer, inner: OperationContext) -> OperationContext {
        let interceptor = MetricsInterceptor {};
        observe::MetricsLayer::new(interceptor).apply_context(srv, inner)
    }
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct MetricsInterceptor;

impl observe::MetricsIntercept for MetricsInterceptor {
    fn observe(&self, labels: observe::MetricLabels, value: observe::MetricValue) {
        let labels = OperationLabels(labels).into_labels();

        match value {
            observe::MetricValue::OperationBytes(v) => {
                histogram!(value.name(), labels).record(v as f64)
            }
            observe::MetricValue::OperationBytesRate(v) => {
                histogram!(value.name(), labels).record(v)
            }
            observe::MetricValue::OperationEntries(v) => {
                histogram!(value.name(), labels).record(v as f64)
            }
            observe::MetricValue::OperationEntriesRate(v) => {
                histogram!(value.name(), labels).record(v)
            }
            observe::MetricValue::OperationDurationSeconds(v) => {
                histogram!(value.name(), labels).record(v)
            }
            observe::MetricValue::OperationErrorsTotal => {
                counter!(value.name(), labels).increment(1)
            }
            observe::MetricValue::OperationExecuting(v) => {
                gauge!(value.name(), labels).increment(v as f64)
            }
            observe::MetricValue::OperationTtfbSeconds(v) => {
                histogram!(value.name(), labels).record(v)
            }

            observe::MetricValue::HttpExecuting(v) => {
                gauge!(value.name(), labels).increment(v as f64)
            }
            observe::MetricValue::HttpRequestBytes(v) => {
                histogram!(value.name(), labels).record(v as f64)
            }
            observe::MetricValue::HttpRequestBytesRate(v) => {
                histogram!(value.name(), labels).record(v)
            }
            observe::MetricValue::HttpRequestDurationSeconds(v) => {
                histogram!(value.name(), labels).record(v)
            }
            observe::MetricValue::HttpResponseBytes(v) => {
                histogram!(value.name(), labels).record(v as f64)
            }
            observe::MetricValue::HttpResponseBytesRate(v) => {
                histogram!(value.name(), labels).record(v)
            }
            observe::MetricValue::HttpResponseDurationSeconds(v) => {
                histogram!(value.name(), labels).record(v)
            }
            observe::MetricValue::HttpConnectionErrorsTotal => {
                counter!(value.name(), labels).increment(1)
            }
            observe::MetricValue::HttpStatusErrorsTotal => {
                counter!(value.name(), labels).increment(1)
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct OperationLabels(observe::MetricLabels);

impl OperationLabels {
    fn into_labels(self) -> Vec<Label> {
        let mut labels = Vec::with_capacity(6);

        labels.extend([
            Label::new(observe::LABEL_SCHEME, self.0.scheme),
            Label::new(observe::LABEL_NAMESPACE, self.0.namespace),
            Label::new(observe::LABEL_ROOT, self.0.root),
            Label::new(observe::LABEL_OPERATION, self.0.operation),
        ]);

        if let Some(error) = self.0.error {
            labels.push(Label::new(observe::LABEL_ERROR, error.into_static()));
        }

        if let Some(status_code) = self.0.status_code {
            labels.push(Label::new(
                observe::LABEL_STATUS_CODE,
                status_code.as_str().to_owned(),
            ));
        }

        if let Some(service_operation) = self.0.service_operation {
            labels.push(Label::new(
                observe::LABEL_SERVICE_OPERATION,
                service_operation,
            ));
        }

        labels
    }
}
