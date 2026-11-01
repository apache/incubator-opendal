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

use super::core::VercelBlobCore;
use super::core::parse_blob;
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::oio::Entry;
use opendal_core::raw::*;

pub struct VercelBlobLister {
    core: Arc<VercelBlobCore>,
    ctx: OperationContext,

    path: String,
    limit: Option<usize>,
}

impl VercelBlobLister {
    pub(super) fn new(
        core: Arc<VercelBlobCore>,
        ctx: OperationContext,
        path: &str,
        limit: Option<usize>,
    ) -> Self {
        VercelBlobLister {
            core,
            ctx,
            path: path.to_string(),
            limit,
        }
    }
}

impl oio::PageList for VercelBlobLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let p = build_abs_path(&self.core.root, &self.path);

        let cursor = if ctx.token.is_empty() {
            None
        } else {
            Some(ctx.token.as_str())
        };

        let resp = self.core.list(&self.ctx, &p, self.limit, cursor).await?;

        ctx.done = !resp.has_more;

        match resp.cursor {
            Some(cursor) => ctx.token = cursor,
            // More pages were reported but nothing was handed back to resume from. Stopping
            // truncates the listing; the alternative is re-requesting the same page forever.
            None => ctx.done = true,
        }

        for blob in resp.blobs {
            let path = build_rel_path(&self.core.root, &blob.pathname);

            if path == self.path {
                continue;
            }

            let md = parse_blob(&blob)?;

            ctx.entries.push_back(Entry::new(&path, md));
        }

        Ok(())
    }
}
