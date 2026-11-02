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

#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]

/// URI scheme used for service registration and scheme-driven construction.
pub const AZBLOB_SCHEME: &str = "azblob";

/// Register this service's URI scheme or schemes with an operator registry.
///
/// Registration enables scheme-driven construction through
/// [`opendal_core::Operator::from_uri`] and
/// [`opendal_core::Operator::via_iter`]. Direct construction through
/// [`opendal_core::Operator::new`] does not require registration.
pub fn register_azblob_service(registry: &opendal_core::OperatorRegistry) {
    registry.register::<Azblob>(AZBLOB_SCHEME);
}

mod backend;
mod config;
mod copier;
#[doc(hidden)]
pub mod core;
mod deleter;
mod lister;
mod reader;
#[doc(hidden)]
pub mod writer;

pub use backend::AzblobBuilder as Azblob;
pub use config::AzblobConfig;

/// Re-export of the [`reqsign_azure_storage`] crate that this service signs
/// requests with.
///
/// [`Azblob::credential_provider_chain`] takes a chain of providers for
/// [`reqsign_azure_storage::Credential`]. Implement providers against this
/// re-export and [`opendal_core::reqsign_core`] rather than depending on the
/// `reqsign` crates directly, so that the types always match the ones this
/// service is built with.
pub use reqsign_azure_storage;
