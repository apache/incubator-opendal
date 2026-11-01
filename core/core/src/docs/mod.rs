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

//! OpenDAL contracts, Rust core guides, and historical project documents.
//!
//! Start with [`concepts`] for the Rust core API model. [`specs`] defines the
//! current portable contracts implemented by OpenDAL. Service and layer authors
//! can continue with [`internals`], while [`performance`] covers application
//! tuning. Accepted RFCs in [`rfcs`] preserve the decisions and context that
//! led to current designs.

#![allow(rustdoc::bare_urls)]

pub mod concepts;
pub mod internals;
pub mod performance;
pub mod specs;

/// Changes log for all OpenDAL released versions.
#[doc = include_str!("../../CHANGELOG.md")]
#[cfg(not(doctest))]
#[allow(rustdoc::broken_intra_doc_links, rustdoc::invalid_rust_codeblocks)]
pub mod changelog {}

#[cfg(not(doctest))]
#[allow(rustdoc::broken_intra_doc_links, rustdoc::invalid_rust_codeblocks)]
pub mod rfcs;

/// Upgrade and migrate procedures while OpenDAL meets breaking changes.
#[doc = include_str!("upgrade.md")]
#[cfg(not(doctest))]
#[allow(rustdoc::broken_intra_doc_links, rustdoc::invalid_rust_codeblocks)]
pub mod upgrade {}
