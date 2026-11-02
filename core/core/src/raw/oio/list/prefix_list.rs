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

/// PrefixLister is used to filter entries by prefix.
///
/// For example, if we have a lister that returns entries:
///
/// ```txt
/// .
/// ├── file_a
/// └── file_b
/// ```
///
/// We can use `PrefixLister` to filter entries with prefix `file_`.
pub struct PrefixLister<L> {
    lister: L,
    prefix: String,
}

/// # Safety
///
/// We will only take `&mut Self` reference for FsLister.
unsafe impl<L> Sync for PrefixLister<L> {}

impl<L> PrefixLister<L> {
    /// Create a new flat lister
    pub fn new(lister: L, prefix: &str) -> PrefixLister<L> {
        PrefixLister {
            lister,
            prefix: prefix.to_string(),
        }
    }
}

impl<L> oio::List for PrefixLister<L>
where
    L: oio::List,
{
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        loop {
            match self.lister.next().await {
                Ok(Some(e)) if !e.path().starts_with(&self.prefix) => continue,
                v => return v,
            }
        }
    }
}
