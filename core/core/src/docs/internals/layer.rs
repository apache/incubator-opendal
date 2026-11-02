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

//! Implementing a layer.
//!
//! A [`Layer`] intercepts an operator's composed service, operation context,
//! or both. Use a layer for behavior that applies across services, such as
//! retry, timeout, tracing, metrics, or runtime resource replacement.
//!
//! # Two composition hooks
//!
//! [`Layer`] exposes two hooks:
//!
//! ```text
//! fn apply_service(&self, service: Servicer) -> Servicer;
//! fn apply_context(
//!     &self,
//!     service: Servicer,
//!     context: OperationContext,
//! ) -> OperationContext;
//! ```
//!
//! `apply_service` wraps storage operations. `apply_context` wraps or replaces
//! runtime resources such as the HTTP transport and executor. Each hook
//! returns its input unchanged by default, so a layer implements only the
//! plane it owns.
//!
//! The operator first applies every service hook in insertion order. It then
//! applies every context hook in the same order, passing the final service
//! stack to each context hook. Adding a layer or replacing the base context
//! replays the complete layer list, producing a service stack and context from
//! the same ordering.
//!
//! # Operation layers
//!
//! An operation layer normally contains:
//!
//! - An `XxxLayer` that implements [`Layer::apply_service`].
//! - An `XxxService` that stores the inner [`Servicer`] and implements
//!   [`Service`].
//!
//! The wrapper overrides only the operations it owns and forwards the rest to
//! the inner service. It must also return capabilities that describe the
//! behavior of the wrapped stack. The wrapper keeps its own operation body
//! types concrete until OpenDAL erases it back into a [`Servicer`].
//!
//! # Resource layers
//!
//! A resource-only layer implements [`Layer::apply_context`]. It should
//! preserve the previous resource when lower layers must remain effective. A
//! layer that wraps an HTTP transport or executor must decide explicitly
//! whether requests continue through the previous value or replace it
//! entirely.
//!
//! Layers that coordinate policy across operation and I/O phases can implement
//! both hooks. Shared mutable state requires interior mutability and must
//! remain `Send` and `Sync`, because cloned operators can run operations
//! concurrently.
//!
//! [`Layer`]: crate::raw::Layer
//! [`Layer::apply_service`]: crate::raw::Layer::apply_service
//! [`Layer::apply_context`]: crate::raw::Layer::apply_context
//! [`Service`]: crate::raw::Service
//! [`Servicer`]: crate::raw::Servicer
