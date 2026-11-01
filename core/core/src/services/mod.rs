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

//! Built-in service support.
//!
//! [`Memory`] is the only storage service implemented by `opendal-core`
//! directly. Other services live in `opendal-service-*` crates and are
//! re-exported by the `opendal` facade behind `services-*` Cargo features.
//!
//! Pass a builder directly to [`crate::Operator::new`]. Service registration is
//! only required for scheme-driven construction through
//! [`crate::Operator::from_uri`] or [`crate::Operator::via_iter`].

mod memory;
pub use self::memory::*;
