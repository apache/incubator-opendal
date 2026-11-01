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
pub const GCS_GRPC_SCHEME: &str = "gcs-grpc";

/// Register the Google Cloud Storage gRPC service with an operator registry.
pub fn register_gcs_grpc_service(registry: &opendal_core::OperatorRegistry) {
    registry.register::<GcsGrpc>(GCS_GRPC_SCHEME);
}

mod backend;
mod config;
mod copier;
mod core;
mod deleter;
#[allow(dead_code, missing_docs, clippy::all)]
mod generated;
mod lister;
mod reader;
mod writer;

pub use backend::GcsGrpcBuilder as GcsGrpc;
pub use config::GcsGrpcConfig;

/// Re-export of the [`reqsign_google`] crate that this service signs requests
/// with.
///
/// [`GcsGrpc::credential_provider`] and [`GcsGrpc::credential_provider_chain`]
/// take providers for [`reqsign_google::Credential`]. Implement providers against
/// this re-export and [`opendal_core::reqsign_core`] rather than depending on the
/// `reqsign` crates directly, so that the types always match the ones this
/// service is built with.
pub use reqsign_google;
