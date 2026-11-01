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

use crate::Copier as AsyncCopier;
use crate::*;

/// BlockingCopier is designed to drive long-running copy operations in a
/// blocking manner.
pub struct Copier {
    handle: tokio::runtime::Handle,
    copier: Option<AsyncCopier>,
}

/// # Safety
///
/// BlockingCopier will only be accessed by `&mut Self`.
unsafe impl Sync for Copier {}

impl Copier {
    /// Create a new blocking copier.
    pub(crate) fn new(handle: tokio::runtime::Handle, copier: AsyncCopier) -> Self {
        Self {
            handle,
            copier: Some(copier),
        }
    }

    /// Abort the pending copy operation.
    pub fn abort(&mut self) -> Result<()> {
        let Some(copier) = self.copier.as_mut() else {
            return Ok(());
        };

        self.handle.block_on(copier.abort())
    }
}

impl Iterator for Copier {
    type Item = Result<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        let copier = self.copier.as_mut()?;

        match self.handle.block_on(copier.next()) {
            Ok(Some(n)) => Some(Ok(n)),
            Ok(None) => {
                self.copier = None;
                None
            }
            Err(err) => Some(Err(err)),
        }
    }
}

impl Drop for Copier {
    fn drop(&mut self) {
        if let Some(v) = self.copier.take() {
            self.handle.block_on(async move { drop(v) });
        }
    }
}
