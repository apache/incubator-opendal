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

use std::sync::Arc;

use super::core::GistCore;
use super::core::parse_metadata;
use opendal_core::raw::*;
use opendal_core::*;

/// Lister returned by this backend.
pub struct GistLister {
    core: Arc<GistCore>,
    ctx: OperationContext,
    path: String,
}

impl GistLister {
    pub(super) fn new(core: Arc<GistCore>, ctx: OperationContext, path: String) -> Self {
        Self { core, ctx, path }
    }
}

impl oio::PageList for GistLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        ctx.done = true;

        // A gist is a flat set of files, so only the root holds entries.
        if !self.path.trim_matches('/').is_empty() {
            return Ok(());
        }

        let Some(gist) = self.core.get_gist(&self.ctx, Operation::List).await? else {
            return Ok(());
        };

        let mut files = gist.files.into_iter().collect::<Vec<_>>();
        files.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, file) in files {
            ctx.entries
                .push_back(oio::Entry::new(&name, parse_metadata(&file)));
        }

        Ok(())
    }
}
