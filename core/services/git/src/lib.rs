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

/// URI scheme registered for the Git service.
pub const GIT_SCHEME: &str = "git";

/// Register the Git service with an operator registry.
///
/// Registration enables scheme-driven construction through
/// [`opendal_core::Operator::via_iter`]. The service does not implement URI
/// parsing, so `from_uri` remains unsupported.
pub fn register_git_service(registry: &opendal_core::OperatorRegistry) {
    registry.register::<Git>(GIT_SCHEME);
}

mod backend;
mod config;
mod core;
mod lister;
mod reader;

pub use backend::GitBuilder as Git;
pub use config::GitConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_is_git() {
        assert_eq!(GIT_SCHEME, "git");
    }
}
