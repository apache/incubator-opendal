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
use crate::options::DeleteOptions;

/// Converts a value into an owned path and logical delete options.
///
/// [`Deleter`](crate::Deleter) lowers the returned options against the composed
/// service capability immediately before passing raw arguments to the service.
pub trait IntoDeleteInput: Send + Sync + Unpin {
    /// Convert `self` into an owned path and delete options.
    fn into_delete_input(self) -> (String, DeleteOptions);
}

/// Implement `IntoDeleteInput` for `&str` so path streams can borrow their items.
impl IntoDeleteInput for &str {
    fn into_delete_input(self) -> (String, DeleteOptions) {
        (self.to_owned(), DeleteOptions::default())
    }
}

/// Implement `IntoDeleteInput` for `String` so `Vec<String>` can be deleted directly.
impl IntoDeleteInput for String {
    fn into_delete_input(self) -> (String, DeleteOptions) {
        (self, DeleteOptions::default())
    }
}

/// Implement `IntoDeleteInput` for an owned path with logical delete options.
impl IntoDeleteInput for (String, DeleteOptions) {
    fn into_delete_input(self) -> (String, DeleteOptions) {
        self
    }
}

/// Implement `IntoDeleteInput` for `Entry` so a lister can feed a deleter.
impl IntoDeleteInput for Entry {
    fn into_delete_input(self) -> (String, DeleteOptions) {
        let (path, meta) = self.into_parts();
        let options = DeleteOptions {
            version: meta.version().map(str::to_owned),
            ..Default::default()
        };
        (path, options)
    }
}
