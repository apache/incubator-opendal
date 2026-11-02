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

/// URI scheme used for memory service registration and URI construction.
pub const MEMORY_SCHEME: &str = "memory";

mod backend;
mod config;
mod core;
mod deleter;
mod lister;
mod writer;

pub use backend::MemoryBuilder as Memory;
pub use config::MemoryConfig;

/// Register the memory URI scheme with an operator registry.
///
/// Registration enables scheme-driven construction through
/// [`crate::Operator::from_uri`] and [`crate::Operator::via_iter`]. Direct
/// construction through [`crate::Operator::new`] does not require registration.
pub fn register_memory_service(registry: &crate::OperatorRegistry) {
    registry.register::<Memory>(MEMORY_SCHEME);
}
