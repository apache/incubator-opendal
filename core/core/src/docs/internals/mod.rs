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

//! The internal implementation details of OpenDAL.
//!
//! This guide explains the extension boundaries used to implement OpenDAL
//! storage services and layers. Application code should use the public
//! [`Operator`] API; the raw APIs described here can change between minor
//! releases.
//!
//! OpenDAL keeps service implementations typed until it composes them into an
//! operator:
//!
//! ```text
//! Builder -> Service -> ServiceDyn -> Servicer -> Layer stack -> Operator
//!              ^                          |
//!              +------ type erasure ------+
//! ```
//!
//! - A [`Service`] implements storage operations and reports the capabilities
//!   it provides.
//! - [`ServiceDyn`] is the object-safe boundary that erases a typed service
//!   and its operation bodies.
//! - A [`Servicer`] is the shared, type-erased service handle stored by
//!   operators and layers.
//! - A [`Layer`] wraps the service stack, the operation context, or both.
//!
//! Continue with [implementing a service][service] or
//! [implementing a layer][layer].
//!
//! [`Operator`]: crate::Operator
//! [`Service`]: crate::raw::Service
//! [`ServiceDyn`]: crate::raw::ServiceDyn
//! [`Servicer`]: crate::raw::Servicer
//! [`Layer`]: crate::raw::Layer

pub mod layer;
pub mod service;
