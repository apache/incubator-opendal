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

/// A layer that guesses MIME types for objects based on their paths or content.
///
/// This layer uses the `mime_guess` crate
/// (see https://crates.io/crates/mime_guess) to infer the
/// ``Content-Type``.
///
/// Notes
/// -----
/// This layer will not override a ``Content-Type`` that has already
/// been set, either manually or by the backend service. It is only
/// applied if no content type is present.
///
/// A ``Content-Type`` is not guaranteed. If the file extension is
/// uncommon or unknown, the content type will remain unset.
#[pyclass(module = "opendal.layers", extends = Layer, skip_from_py_object)]
#[derive(Clone)]
pub struct MimeGuessLayer(ocore::layers::MimeGuessLayer);

impl PythonLayer for MimeGuessLayer {
    fn layer(&self, op: Operator) -> Operator {
        op.layer(self.0.clone())
    }
}
#[pymethods]
impl MimeGuessLayer {
    /// Create a new MimeGuessLayer.
    ///
    /// Returns
    /// -------
    /// MimeGuessLayer
    #[new]
    fn new() -> PyResult<PyClassInitializer<Self>> {
        let mime_guess = Self(ocore::layers::MimeGuessLayer::default());
        let class =
            PyClassInitializer::from(Layer(Box::new(mime_guess.clone()))).add_subclass(mime_guess);
        Ok(class)
    }
}
