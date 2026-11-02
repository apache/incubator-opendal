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

use super::Layer;
use super::PythonLayer;
use crate::*;
use opendal::Operator;

/// A layer that overrides the full capability exposed by an operator.
#[pyclass(module = "opendal.layers", extends=Layer, skip_from_py_object)]
#[derive(Clone)]
pub struct CapabilityOverrideLayer(ocore::layers::CapabilityOverrideLayer);

impl PythonLayer for CapabilityOverrideLayer {
    fn layer(&self, op: Operator) -> Operator {
        op.layer(self.0.clone())
    }
}
#[pymethods]
impl CapabilityOverrideLayer {
    /// Create a new CapabilityOverrideLayer from capability override entries.
    ///
    /// Parameters
    /// ----------
    /// overrides : str
    ///     Comma-separated capability override entries.
    ///
    /// Returns
    /// -------
    /// CapabilityOverrideLayer
    #[new]
    #[pyo3(signature = (overrides))]
    fn new(overrides: &str) -> PyResult<PyClassInitializer<Self>> {
        let layer = Self(
            ocore::layers::CapabilityOverrideLayer::from_overrides(overrides)
                .map_err(format_pyerr)?,
        );
        let class = PyClassInitializer::from(Layer(Box::new(layer.clone()))).add_subclass(layer);

        Ok(class)
    }
}
