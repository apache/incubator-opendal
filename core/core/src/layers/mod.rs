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

//! Core layers used by `opendal-core`.
//!
//! A [`crate::raw::Layer`] wraps an access implementation to intercept or adjust
//! operations. Optional reusable layers such as retry, timeout, logging, and
//! metrics live in `opendal-layer-*` crates and are re-exported by the
//! `opendal` facade behind `layers-*` Cargo features.

mod error_context;
pub(crate) use error_context::ErrorContextLayer;

mod complete;
pub(crate) use complete::CompleteLayer;

mod simulate;
pub use simulate::SimulateLayer;

mod capability_override;
pub use capability_override::CapabilityOverrideLayer;

mod correctness_check;
pub(crate) use correctness_check::CorrectnessCheckLayer;
