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

#![allow(
    rustdoc::broken_intra_doc_links,
    reason = "YARD's syntax for documentation"
)]

use magnus::Error;
use magnus::RModule;
use magnus::method;

use crate::capability::Capability;
use crate::*;

/// Metadata for operator, users can use this metadata to get information of operator.
#[magnus::wrap(class = "OpenDal::OperatorInfo", free_immediately, size)]
pub struct OperatorInfo(pub ocore::OperatorInfo);

impl OperatorInfo {
    /// @yard
    /// @def scheme
    /// Returns the scheme string of the operator.
    /// @return [String]
    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    /// @yard
    /// @def root
    /// Returns the root path of the operator, will be in format like `/path/to/dir/`
    /// @return [String]
    pub fn root(&self) -> String {
        self.0.root()
    }

    /// @yard
    /// @def name
    /// Returns the name of backend, could be empty if underlying backend doesn't have namespace concept.
    ///
    /// For example:
    ///
    /// - name for `s3` => bucket name
    /// - name for `azblob` => container name
    ///
    /// @return [String]
    pub fn name(&self) -> String {
        self.0.name()
    }

    /// @yard
    /// @def capability
    /// Returns the [`Full Capability`] of the operator.
    /// @return [Capability]
    pub fn capability(&self) -> Capability {
        Capability::new(self.0.capability())
    }
}

pub fn include(ruby: &Ruby, gem_module: &RModule) -> Result<(), Error> {
    let class = gem_module.define_class("OperatorInfo", ruby.class_object())?;
    class.define_method("scheme", method!(OperatorInfo::scheme, 0))?;
    class.define_method("root", method!(OperatorInfo::root, 0))?;
    class.define_method("name", method!(OperatorInfo::name, 0))?;
    class.define_method("capability", method!(OperatorInfo::capability, 0))?;

    Ok(())
}
