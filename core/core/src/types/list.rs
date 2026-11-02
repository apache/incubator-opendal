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

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use futures::Stream;

use crate::raw::*;
use crate::*;

/// `Lister` lists entries at a given path asynchronously.
///
/// - `Lister` implements `Stream<Item = Result<Entry>>`.
/// - `Lister` returns `None` after it yields all entries or encounters an error.
pub struct Lister {
    lister: Option<oio::Lister>,

    fut: Option<BoxedStaticFuture<(oio::Lister, Result<Option<oio::Entry>>)>>,
    errored: bool,
}

/// # Safety
///
/// All access to `Lister`'s internal state requires `&mut self`.
unsafe impl Sync for Lister {}

impl Lister {
    /// Create a new lister.
    pub(crate) fn create(
        ctx: OperationContext,
        srv: Servicer,
        path: &str,
        args: OpList,
    ) -> Result<Self> {
        let lister = srv.list(&ctx, path, args)?;

        Ok(Self {
            lister: Some(lister),

            fut: None,
            errored: false,
        })
    }
}

impl Stream for Lister {
    type Item = Result<Entry>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Returns `None` if we encountered an error.
        if self.errored {
            return Poll::Ready(None);
        }

        if let Some(mut lister) = self.lister.take() {
            let fut = async move {
                let res = lister.next_dyn().await;
                (lister, res)
            };
            self.fut = Some(Box::pin(fut));
        }

        if let Some(fut) = self.fut.as_mut() {
            let (lister, entry) = ready!(fut.as_mut().poll(cx));
            self.lister = Some(lister);
            self.fut = None;

            return match entry {
                Ok(Some(oe)) => Poll::Ready(Some(Ok(oe.into_entry()))),
                Ok(None) => {
                    self.lister = None;
                    Poll::Ready(None)
                }
                Err(err) => {
                    self.errored = true;
                    Poll::Ready(Some(Err(err)))
                }
            };
        }

        Poll::Ready(None)
    }
}
