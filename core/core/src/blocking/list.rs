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

use futures::StreamExt;

use crate::Lister as AsyncLister;
use crate::*;

/// BlockingLister is designed to list entries at given path in a blocking
/// manner.
///
/// Users can construct a lister through [`Operator::lister`][crate::blocking::Operator::lister]
/// or [`Operator::lister_options`][crate::blocking::Operator::lister_options].
///
/// - Lister implements `Iterator<Item = Result<Entry>>`.
/// - Lister will return `None` if there is no more entries or error has been returned.
pub struct Lister {
    handle: tokio::runtime::Handle,
    lister: Option<AsyncLister>,
}

/// # Safety
///
/// BlockingLister will only be accessed by `&mut Self`
unsafe impl Sync for Lister {}

impl Lister {
    /// Create a new lister.
    pub(crate) fn new(handle: tokio::runtime::Handle, lister: AsyncLister) -> Self {
        Self {
            handle,
            lister: Some(lister),
        }
    }
}

impl Iterator for Lister {
    type Item = Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(lister) = self.lister.as_mut() else {
            return Some(Err(Error::new(
                ErrorKind::Unexpected,
                "lister has been dropped",
            )));
        };

        self.handle.block_on(lister.next())
    }
}

impl Drop for Lister {
    fn drop(&mut self) {
        if let Some(v) = self.lister.take() {
            self.handle.block_on(async move { drop(v) });
        }
    }
}
