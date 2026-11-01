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

use std::fmt::Debug;

use crate::raw::*;
use crate::*;

/// Layer intercepts an operator's composed runtime resources.
///
/// A layer receives the current stack for each plane and returns the stack that
/// should be used by operators built with this layer. Implementations can wrap
/// the operation service, the operation context (HTTP transport and executor),
/// or both.
///
/// [`Operator`][crate::Operator] applies layers by first composing the service
/// stack with [`Layer::apply_service`], then composing [`OperationContext`] with
/// [`Layer::apply_context`]. The context hook receives the final service stack
/// so resource wrappers can observe service identity or capability when needed.
///
/// Hooks take `&self`, so layers that keep mutable state must use interior
/// mutability. That state must remain `Send` and `Sync` because layers are
/// shared across cloned operators and concurrent operations.
pub trait Layer: Send + Sync + Debug + Unpin + 'static {
    /// Intercept the operation service stack.
    ///
    /// Operation layers should return a service that forwards unchanged
    /// operations to `inner`.
    fn apply_service(&self, srv: Servicer) -> Servicer {
        srv
    }

    /// Intercept the operation context (HTTP transport and executor).
    ///
    /// Return `inner` unchanged for layers that do not affect HTTP requests or
    /// spawned tasks.
    fn apply_context(&self, srv: Servicer, inner: OperationContext) -> OperationContext {
        let _ = srv;
        inner
    }
}
