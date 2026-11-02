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
pub const COS_SCHEME: &str = "cos";

/// Alternative scheme for cos service.
///
/// `cosn://` is the canonical Hadoop / EMR URI scheme used by many
/// big-data ecosystems (Spark, Hive, Flink, etc.) to access Tencent Cloud
/// COS. OpenDAL accepts it as an alias of `cos://` so that existing
/// workloads can switch to OpenDAL without changing their URIs.
pub const COSN_SCHEME: &str = "cosn";

/// Register this service's URI scheme or schemes with an operator registry.
///
/// Registration enables scheme-driven construction through
/// [`opendal_core::Operator::from_uri`] and
/// [`opendal_core::Operator::via_iter`]. Direct construction through
/// [`opendal_core::Operator::new`] does not require registration.
///
/// Both the canonical `cos` scheme and the Hadoop-compatible `cosn`
/// alias are registered so that URIs like `cos://bucket/key` and
/// `cosn://bucket/key` are both resolvable to this backend.
pub fn register_cos_service(registry: &opendal_core::OperatorRegistry) {
    registry.register::<Cos>(COS_SCHEME);
    registry.register::<Cos>(COSN_SCHEME);
}

mod backend;
mod config;
mod core;
mod deleter;
mod lister;
mod reader;
mod writer;

pub use backend::CosBuilder as Cos;
pub use config::CosConfig;
