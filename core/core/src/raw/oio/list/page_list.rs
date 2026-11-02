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

use std::collections::VecDeque;
use std::future::Future;

use crate::raw::*;
use crate::*;

/// PageList is used to implement [`oio::List`] based on API supporting pagination. By implementing
/// PageList, services don't need to care about the details of page list.
///
/// # Architecture
///
/// The architecture after adopting [`PageList`]:
///
/// - Services impl `PageList`
/// - `PageLister` impl `List`
/// - Expose `PageLister` as `Service::Lister`
pub trait PageList: Send + Sync + Unpin + 'static {
    /// next_page is used to fetch next page of entries from underlying storage.
    fn next_page(&self, ctx: &mut PageContext) -> impl Future<Output = Result<()>> + MaybeSend;
}

/// PageContext is the context passing between `PageList`.
///
/// [`PageLister`] will init the PageContext, and implementer of [`PageList`] should fill the `PageContext`
/// based on their needs.
///
/// - Set `done` to `true` if all page have been fetched.
/// - Update `token` if there is more page to fetch. `token` is not exposed to users, it's internal used only.
/// - Push back into the entries for each entry fetched from underlying storage.
///
/// NOTE: `entries` is a `VecDeque` to avoid unnecessary memory allocation. Only `push_back` is allowed.
pub struct PageContext {
    /// done is used to indicate whether the list operation is done.
    pub done: bool,
    /// token is used by underlying storage services to fetch next page.
    pub token: String,
    /// entries are used to store entries fetched from underlying storage.
    ///
    /// Please always reuse the same `VecDeque` to avoid unnecessary memory allocation.
    /// PageLister makes sure that entries is reset before calling `next_page`. Implementer
    /// can call `push_back` on `entries` directly.
    pub entries: VecDeque<oio::Entry>,
}

/// PageLister implements [`oio::List`] based on [`PageList`].
pub struct PageLister<L: PageList> {
    inner: L,
    ctx: PageContext,
}

impl<L> PageLister<L>
where
    L: PageList,
{
    /// Create a new PageLister.
    pub fn new(l: L) -> Self {
        Self {
            inner: l,
            ctx: PageContext {
                done: false,
                token: "".to_string(),
                entries: VecDeque::new(),
            },
        }
    }
}

impl<L> oio::List for PageLister<L>
where
    L: PageList,
{
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        loop {
            if let Some(entry) = self.ctx.entries.pop_front() {
                return Ok(Some(entry));
            }
            if self.ctx.done {
                return Ok(None);
            }

            self.inner.next_page(&mut self.ctx).await?;
        }
    }
}
