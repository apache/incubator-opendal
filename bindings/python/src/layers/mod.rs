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

//! Python layer bindings.
//!
//! Each layer lives in its own module so new layers can be added without growing
//! a single file. The concrete types are re-exported here (so paths such as
//! `crate::RetryLayer` keep working) and registered on the flat `opendal.layers`
//! Python module.

mod capability_override;
mod concurrent_limit;
mod mime_guess;
mod retry;
mod timeout;

pub use capability_override::CapabilityOverrideLayer;
pub use concurrent_limit::ConcurrentLimitLayer;
pub use mime_guess::MimeGuessLayer;
pub use retry::RetryLayer;
pub use timeout::TimeoutLayer;

use opendal::Operator;
use pyo3::prelude::*;

/// Shared behavior for every Python layer: wrap an [`Operator`] with the
/// underlying core layer.
pub trait PythonLayer: Send + Sync {
    fn layer(&self, op: Operator) -> Operator;
}

/// Base class for all layers.
///
/// A layer wraps an operator to intercept operations on the underlying
/// storage, adding behavior such as retries, timeouts, concurrency limits,
/// or content-type detection.
///
/// This class is not meant to be instantiated directly. Use a concrete
/// subclass such as `RetryLayer` or `TimeoutLayer` and apply it with
/// `Operator.layer` or `AsyncOperator.layer`.
#[pyclass(module = "opendal.layers", subclass)]
pub struct Layer(pub Box<dyn PythonLayer>);
