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

#![doc(html_logo_url = "https://opendal.apache.org/img/logo.svg")]
#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
// Make sure all our public APIs have docs.
#![deny(missing_docs)]

// Private module with public types, they will be accessed via `opendal::Xxxx`
mod types;
pub use types::*;

// Public modules, they will be accessed like `opendal::layers::Xxxx`
#[cfg(feature = "blocking")]
pub mod blocking;
pub mod docs;
pub mod layers;
pub mod raw;
pub mod services;

/// Re-export of the [`reqsign_core`] crate that services sign requests with.
///
/// Service builders that accept credentials from the application take `reqsign`
/// types in their signatures, such as `ProvideCredentialChain` in
/// `opendal_service_s3::S3::credential_provider_chain`. Implement providers
/// against this re-export rather than depending on `reqsign-core` directly, so
/// that the types always match the ones the services are built with.
#[cfg(feature = "reqsign")]
pub use reqsign_core;

#[cfg(test)]
mod tests {
    use super::*;

    trait AssertSendSync: Send + Sync {}
    impl AssertSendSync for Entry {}
    impl AssertSendSync for Capability {}
    impl AssertSendSync for Error {}
    impl AssertSendSync for Reader {}
    impl AssertSendSync for Writer {}
    impl AssertSendSync for Lister {}
    impl AssertSendSync for Operator {}

    /// This is used to make sure our public API implement Send + Sync
    #[test]
    fn test_trait() {
        let _: Box<dyn AssertSendSync> = Box::new(Capability::default());
    }
}
