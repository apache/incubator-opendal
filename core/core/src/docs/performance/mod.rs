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

//! OpenDAL Performance Guide
//!
//! OpenDAL keeps the storage abstraction lightweight, but applications still
//! need to configure buffering, concurrency, and HTTP transport for their
//! workload. This guide gives concrete Rust configuration examples and explains
//! the trade-offs behind them.
//!
//! - [Concurrent writes][concurrent_write] explains how OpenDAL schedules write
//!   parts and how to tune `concurrent` and `chunk`.
//! - [HTTP optimization][http_optimization] explains when to try HTTP/1.1 and
//!   how to configure DNS, timeouts, and connection pools.

#[doc = include_str!("concurrent_write.md")]
pub mod concurrent_write {}

#[doc = include_str!("http_optimization.md")]
pub mod http_optimization {}
