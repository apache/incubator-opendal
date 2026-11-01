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

use crate::Entry;
use crate::options::ComposeSourceOptions;

/// Converts a value into an owned source path and its composition options.
pub trait IntoComposeInput: Send + Sync + Unpin {
    /// Convert `self` into a source path and options.
    fn into_compose_input(self) -> (String, ComposeSourceOptions);
}

impl IntoComposeInput for &str {
    fn into_compose_input(self) -> (String, ComposeSourceOptions) {
        (self.to_owned(), ComposeSourceOptions::default())
    }
}

impl IntoComposeInput for String {
    fn into_compose_input(self) -> (String, ComposeSourceOptions) {
        (self, ComposeSourceOptions::default())
    }
}

impl<P> IntoComposeInput for (P, ComposeSourceOptions)
where
    P: Into<String> + Send + Sync + Unpin,
{
    fn into_compose_input(self) -> (String, ComposeSourceOptions) {
        (self.0.into(), self.1)
    }
}

impl IntoComposeInput for Entry {
    fn into_compose_input(self) -> (String, ComposeSourceOptions) {
        let (path, metadata) = self.into_parts();
        let options = ComposeSourceOptions {
            version: metadata.version().map(ToOwned::to_owned),
            ..Default::default()
        };
        (path, options)
    }
}
