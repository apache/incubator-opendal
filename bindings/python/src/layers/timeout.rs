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

/// Parse a timeout given in seconds into a [`Duration`].
///
/// Rejects non-positive, non-finite, or out-of-range values. A zero timeout
/// is rejected because it would make every operation time out immediately.
fn parse_timeout_secs(name: &str, secs: f64) -> PyResult<Duration> {
    if !secs.is_finite() || secs <= 0.0 {
        return Err(ConfigInvalid::new_err(format!(
            "{name} must be a positive, finite number of seconds"
        )));
    }
    Duration::try_from_secs_f64(secs).map_err(|_| {
        ConfigInvalid::new_err(format!(
            "{name} must be a positive, finite number of seconds"
        ))
    })
}

/// A layer that adds timeouts to operations.
///
/// Timeouts prevent slow or stalled work from hanging indefinitely, for
/// example when a TCP connection stops emitting IO events. Control
/// operations (such as `stat` and `delete`) and IO operations (such as
/// `read` and `write`) are bounded by separate timeouts.
///
/// Notes
/// -----
/// A small amount of overhead is added to IO operations to implement the
/// timeout correctly.
#[pyclass(module = "opendal.layers", extends = Layer, skip_from_py_object)]
#[derive(Clone)]
pub struct TimeoutLayer(ocore::layers::TimeoutLayer);

impl PythonLayer for TimeoutLayer {
    fn layer(&self, op: Operator) -> Operator {
        op.layer(self.0.clone())
    }
}
#[pymethods]
impl TimeoutLayer {
    /// Create a new TimeoutLayer.
    ///
    /// Parameters
    /// ----------
    /// timeout : Optional[float]
    ///     Timeout (in seconds) for control operations like ``stat`` and
    ///     ``delete``. Must be a positive, finite number. A value of ``0``
    ///     is rejected because it would make every operation time out
    ///     immediately. Defaults to ``60.0``.
    /// io_timeout : Optional[float]
    ///     Timeout (in seconds) for IO operations like ``read`` and
    ///     ``write``. Must be a positive, finite number. A value of ``0``
    ///     is rejected because it would make every operation time out
    ///     immediately. Defaults to ``10.0``.
    ///
    /// Returns
    /// -------
    /// TimeoutLayer
    ///
    /// Raises
    /// ------
    /// ConfigInvalid
    ///     If ``timeout`` or ``io_timeout`` is out of range.
    #[new]
    #[pyo3(signature = (timeout = None, io_timeout = None))]
    fn new(timeout: Option<f64>, io_timeout: Option<f64>) -> PyResult<PyClassInitializer<Self>> {
        let mut timeout_layer = ocore::layers::TimeoutLayer::default();
        if let Some(timeout) = timeout {
            let timeout = parse_timeout_secs("timeout", timeout)?;
            timeout_layer = timeout_layer.with_timeout(timeout);
        }
        if let Some(io_timeout) = io_timeout {
            let io_timeout = parse_timeout_secs("io_timeout", io_timeout)?;
            timeout_layer = timeout_layer.with_io_timeout(io_timeout);
        }

        let timeout_layer = Self(timeout_layer);
        let class = PyClassInitializer::from(Layer(Box::new(timeout_layer.clone())))
            .add_subclass(timeout_layer);

        Ok(class)
    }
}
