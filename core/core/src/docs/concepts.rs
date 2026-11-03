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

//! The core concepts of OpenDAL's Rust API.
//!
//! OpenDAL gives applications one storage API across services. Rust
//! applications use four concepts: a service describes the storage backend, a
//! builder configures it, an operator exposes operations, and layers add
//! behavior around those operations.
//!
//! For the language-independent model shared by every OpenDAL binding, see the
//! [OpenDAL concepts guide](https://opendal.apache.org/docs/concepts/).
//!
//! # Service and builder
//!
//! A **service** is a storage backend such as S3, Google Cloud Storage, a
//! local filesystem, or an in-memory store. Each service supports a different
//! set of operations and optional features.
//!
//! A [`Builder`] collects one service's configuration and constructs its
//! implementation. The [`services`][crate::services] module exposes concrete
//! builders, while [`Operator::new`] turns a builder into a ready-to-use
//! operator.
//!
//! ```text
//! configuration -> builder -> service -> operator
//! ```
//!
//! ```no_run
//! # use opendal_core::Result;
//! use opendal_core::Operator;
//! use opendal_core::services::Memory;
//!
//! # fn test() -> Result<()> {
//! let builder = Memory::default();
//! let op = Operator::new(builder)?;
//! # let _ = op;
//! # Ok(())
//! # }
//! ```
//!
//! OpenDAL does not make every service support every operation. Applications
//! can inspect the operator's effective [`Capability`] before using an
//! optional operation or option.
//!
//! # Operator
//!
//! An [`Operator`] is the public handle for one configured service and root.
//! It normalizes paths, validates options against effective capabilities, runs
//! the configured layers, and dispatches each operation to the service.
//!
//! Operators are cheap to clone, contain no caller-visible lifetime or service
//! type parameter, and can be shared across threads. A clone refers to the
//! same composed service stack. Methods that add a layer or replace runtime
//! resources return a new operator; existing clones and in-flight operations
//! keep their current stack.
//!
//! # Operation
//!
//! Operations are storage actions such as `read`, `write`, `stat`, `list`,
//! `delete`, `copy`, and `rename`. Convenience methods use default options,
//! while the corresponding `_with` methods expose operation-specific options.
//!
//! ```no_run
//! # use opendal_core::Result;
//! use opendal_core::Operator;
//! use opendal_core::services::Memory;
//!
//! # async fn test() -> Result<()> {
//! let op = Operator::new(Memory::default())?;
//! let bs = op.read("abc").await?;
//! # let _ = bs;
//! # Ok(())
//! # }
//! ```
//!
//! OpenDAL normalizes every path relative to the operator's root. `/`
//! represents the root, a trailing `/` represents a directory, and any other
//! normalized path represents a file. Operation documentation defines the
//! observable behavior and errors. [Specifications][super::specs] define
//! portable contracts that span multiple operations and services.
//!
//! # Layer and operation context
//!
//! A [`Layer`][crate::raw::Layer] adds cross-cutting behavior such as retry,
//! timeout, tracing, or metrics. Layers form an ordered stack around a
//! service.
//!
//! An [`OperationContext`] carries runtime resources such as the HTTP
//! transport and executor from the operator to the service.
//! Operation-specific values such as ranges, versions, conditions, and
//! concurrency remain in that operation's options.
//!
//! Adding a layer with [`Operator::layer`] or replacing the base context with
//! [`Operator::with_context`] rebuilds both the service stack and the composed
//! context from the same ordered layer list:
//!
//! ```text
//! base service -----+                    +-> composed service --+
//!                   +-> ordered layers --+                      +-> operation
//! base context -----+                    +-> composed context --+
//! ```
//!
//! ```no_run
//! # use opendal_core::Result;
//! use opendal_core::HttpTransporter;
//! use opendal_core::OperationContext;
//! use opendal_core::Operator;
//! use opendal_core::services::Memory;
//!
//! # fn test() -> Result<()> {
//! let transport = HttpTransporter::default();
//! let op = Operator::new(Memory::default())?.with_context(
//!     OperationContext::new().with_http_transport(transport),
//! );
//! # let _ = op;
//! # Ok(())
//! # }
//! ```
//!
//! Most applications only need the public [`Operator`] API. Service and layer
//! authors should continue with the [internals][super::internals] guide.
//!
//! [`Builder`]: crate::Builder
//! [`Operator`]: crate::Operator
//! [`Operator::new`]: crate::Operator::new
//! [`Operator::layer`]: crate::Operator::layer
//! [`Operator::with_context`]: crate::Operator::with_context
//! [`Capability`]: crate::Capability
//! [`OperationContext`]: crate::OperationContext
