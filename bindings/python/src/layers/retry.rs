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

use super::Layer;
use super::PythonLayer;
use crate::*;
use opendal::Operator;

/// A layer that retries operations that fail with temporary errors.
///
/// Operations are retried if they fail with an error for which
/// `Error.is_temporary` returns `True`. If all retries are exhausted,
/// the error is marked as persistent and then returned.
///
/// Notes
/// -----
/// After an operation on a `Reader` or `Writer` has failed through
/// all retries, the object is in an undefined state. Reusing it
/// can lead to exceptions.
#[pyclass(module = "opendal.layers", extends=Layer, skip_from_py_object)]
#[derive(Clone)]
pub struct RetryLayer(ocore::layers::RetryLayer);

impl PythonLayer for RetryLayer {
    fn layer(&self, op: Operator) -> Operator {
        op.layer(self.0.clone())
    }
}
#[pymethods]
impl RetryLayer {
    /// Create a new RetryLayer.
    ///
    /// Parameters
    /// ----------
    /// max_times : Optional[int]
    ///     Maximum number of retry attempts. Defaults to ``3``.
    /// factor : Optional[float]
    ///     Backoff factor applied between retries. Must be a finite value
    ///     ``>= 1.0``. Defaults to ``2.0``.
    /// jitter : bool
    ///     Whether to apply jitter to the backoff. Defaults to ``False``.
    /// max_delay : Optional[float]
    ///     Maximum delay (in seconds) between retries. Must be finite and
    ///     non-negative. Defaults to ``60.0``.
    /// min_delay : Optional[float]
    ///     Minimum delay (in seconds) between retries. Must be finite and
    ///     non-negative. Defaults to ``1.0``.
    ///
    /// Returns
    /// -------
    /// RetryLayer
    ///
    /// Raises
    /// ------
    /// ConfigInvalid
    ///     If ``factor``, ``max_delay``, or ``min_delay`` is out of range.
    #[new]
    #[pyo3(signature = (
        max_times = None,
        factor = None,
        jitter = false,
        max_delay = None,
        min_delay = None
    ))]
    fn new(
        max_times: Option<usize>,
        factor: Option<f32>,
        jitter: bool,
        max_delay: Option<f64>,
        min_delay: Option<f64>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut retry = ocore::layers::RetryLayer::default();
        if let Some(max_times) = max_times {
            retry = retry.with_max_times(max_times);
        }
        if let Some(factor) = factor {
            if !factor.is_finite() || factor < 1.0 {
                return Err(ConfigInvalid::new_err(
                    "factor must be a finite value greater than or equal to 1.0",
                ));
            }
            retry = retry.with_factor(factor);
        }
        if jitter {
            retry = retry.with_jitter();
        }
        if let Some(max_delay) = max_delay {
            let max_delay = Duration::try_from_secs_f64(max_delay).map_err(|_| {
                ConfigInvalid::new_err("max_delay must be a finite, non-negative number of seconds")
            })?;
            retry = retry.with_max_delay(max_delay);
        }
        if let Some(min_delay) = min_delay {
            let min_delay = Duration::try_from_secs_f64(min_delay).map_err(|_| {
                ConfigInvalid::new_err("min_delay must be a finite, non-negative number of seconds")
            })?;
            retry = retry.with_min_delay(min_delay);
        }

        let retry_layer = Self(retry);
        let class = PyClassInitializer::from(Layer(Box::new(retry_layer.clone())))
            .add_subclass(retry_layer);

        Ok(class)
    }
}
