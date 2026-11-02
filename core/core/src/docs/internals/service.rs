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

//! Implementing a service.
//!
//! Every OpenDAL backend implements the raw [`Service`] trait. A service owns
//! protocol-specific request construction and response handling. The operator
//! owns path normalization, option validation, layer composition, and
//! dispatch.
//!
//! Service crates live under `core/services/<name>/`. Their public builder and
//! configuration types construct a typed backend; the backend then implements
//! [`Service`].
//!
//! # Service identity and capabilities
//!
//! [`Service::info`] returns immutable identity such as the scheme, root, and
//! namespace name. Runtime resources do not belong in [`ServiceInfo`];
//! services read them from [`OperationContext`].
//!
//! [`Service::capability`] reports the behavior implemented by that service
//! stack. Set an operation or option capability only when the implementation
//! satisfies the corresponding public contract. Operators reject options whose
//! required capability is absent.
//!
//! # Operation body types
//!
//! [`Service`] uses associated types for operation bodies:
//!
//! ```text
//! type Reader: oio::Read;
//! type Writer: oio::Write;
//! type Lister: oio::List;
//! type Deleter: oio::Delete;
//! type Copier: oio::Copy;
//! type Composer: oio::Compose;
//! ```
//!
//! A backend returns concrete body types so its implementation and typed
//! wrappers do not pay for dynamic dispatch. Use `()` for an unsupported body
//! type and return [`ErrorKind::Unsupported`] from the corresponding operation
//! entry point.
//!
//! OpenDAL erases these types once, at [`ServiceDyn`]. [`Servicer`] is
//! `Arc<dyn ServiceDyn>` and is the handle used by operators and runtime layer
//! composition. A wrapper that receives a [`Servicer`] may forward erased
//! `oio::*` bodies, but a backend should keep its own bodies concrete.
//!
//! # Operation methods
//!
//! Each operation method receives normalized paths, an [`OperationContext`],
//! and operation-specific arguments. The context supplies layer-composed
//! runtime resources such as the HTTP transport and executor. Options such as
//! ranges, versions, conditions, and concurrency remain in the operation
//! arguments.
//!
//! An implementation must:
//!
//! - Map every advertised option to the native request without silently
//!   dropping it.
//! - Preserve the operation's public success, error, and atomicity contract.
//! - Return structured OpenDAL errors with the correct [`ErrorKind`] and
//!   useful context.
//! - Keep credentials and other secrets out of `Debug` output and errors.
//! - Forward cancellation and cleanup to protocol-specific readers, writers,
//!   deleters, and copiers.
//!
//! # Adding or changing a service
//!
//! Keep configuration, request construction, operation bodies, and error
//! parsing at their existing service boundaries. Update the facade feature and
//! service registration when the service must be available through the
//! `opendal` crate.
//!
//! Before advertising new behavior, reproduce it against the actual service
//! and run the matching capability-gated behavior tests. Protocol
//! documentation, emulators, and fabricated responses can explain an
//! implementation, but they do not establish real-service conformance.
//!
//! [`Service`]: crate::raw::Service
//! [`Service::info`]: crate::raw::Service::info
//! [`Service::capability`]: crate::raw::Service::capability
//! [`ServiceInfo`]: crate::raw::ServiceInfo
//! [`ServiceDyn`]: crate::raw::ServiceDyn
//! [`Servicer`]: crate::raw::Servicer
//! [`OperationContext`]: crate::OperationContext
//! [`ErrorKind`]: crate::ErrorKind
//! [`ErrorKind::Unsupported`]: crate::ErrorKind::Unsupported
