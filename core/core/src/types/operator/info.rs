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

use crate::raw::*;
use crate::*;

/// Metadata for operator, users can use this metadata to get information of operator.
#[derive(Clone, Debug)]
pub struct OperatorInfo {
    info: ServiceInfo,
    capability: Capability,
}

impl OperatorInfo {
    pub(super) fn new(info: ServiceInfo, capability: Capability) -> Self {
        Self { info, capability }
    }

    /// Return the operator's scheme.
    pub fn scheme(&self) -> &'static str {
        self.info.scheme()
    }

    /// Return the operator's root in a format like `/path/to/dir/`.
    pub fn root(&self) -> String {
        self.info.root().to_string()
    }

    /// Return the operator's name.
    ///
    /// The name can be empty if the service has no namespace concept.
    ///
    /// For example:
    ///
    /// - name for `s3` => bucket name
    /// - name for `azblob` => container name
    pub fn name(&self) -> String {
        self.info.name().to_string()
    }

    /// Return the operator's effective capabilities.
    pub fn capability(&self) -> Capability {
        self.capability
    }
}
