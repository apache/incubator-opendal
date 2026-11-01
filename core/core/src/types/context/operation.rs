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

use crate::*;

/// Composed resources passed from operator to services and layers.
///
/// [`Operator`][crate::Operator] keeps a base `OperationContext` and replays its
/// layers to produce the composed context passed to every service operation.
/// The context carries runtime resources supplied by the operator stack, not
/// caller intent for a specific operation. Operation-specific inputs live in
/// `Op*` argument structs such as `OpRead` and `OpWrite`.
///
/// Service implementations should read HTTP transport, executor, and similar
/// resources from the `OperationContext` they receive at the operation boundary.
/// They should not cache those resources in the service created by
/// [`Builder::build`], because later layers or [`Operator::with_context`] can
/// replace them for a derived operator.
///
/// Layers that replace the HTTP transport or executor must keep forwarding
/// requests or tasks to the previous value. Otherwise, composed features such as
/// retry, timeout, tracing, and metrics can be bypassed.
#[derive(Clone, Debug)]
pub struct OperationContext {
    http_transport: HttpTransporter,
    executor: Executor,
}

impl OperationContext {
    /// Create a new operation context with default runtime resources.
    pub fn new() -> Self {
        Self::from_parts(HttpTransporter::default(), Executor::default())
    }

    /// Create a new operation context from composed runtime resources.
    pub fn from_parts(http_transport: HttpTransporter, executor: Executor) -> Self {
        Self {
            http_transport,
            executor,
        }
    }

    /// Return a copy of this context with a different HTTP transport.
    pub fn with_http_transport(&self, http_transport: HttpTransporter) -> Self {
        Self {
            http_transport,
            executor: self.executor.clone(),
        }
    }

    /// Return a copy of this context with a different executor.
    pub fn with_executor(&self, executor: Executor) -> Self {
        Self {
            http_transport: self.http_transport.clone(),
            executor,
        }
    }

    /// Split into composed resources.
    pub fn into_parts(self) -> (HttpTransporter, Executor) {
        (self.http_transport, self.executor)
    }

    /// Get the composed HTTP transport.
    pub fn http_transport(&self) -> &HttpTransporter {
        &self.http_transport
    }

    /// Get the composed executor.
    pub fn executor(&self) -> &Executor {
        &self.executor
    }
}

impl Default for OperationContext {
    fn default() -> Self {
        Self::new()
    }
}
