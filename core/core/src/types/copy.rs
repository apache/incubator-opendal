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
use std::{fmt, fmt::Formatter};

use futures::Stream;

use crate::raw::*;
use crate::*;

/// `Copier` drives a long-running copy operation one step at a time.
///
/// - `Copier` implements `Stream<Item = Result<usize>>`.
/// - `Some(Ok(n))` means the copy operation made progress by `n` bytes.
/// - `None` means the copy operation has completed.
pub struct Copier {
    copier: Option<oio::Copier>,
    metadata: Option<Metadata>,

    fut: Option<BoxedStaticFuture<(oio::Copier, Result<Option<usize>>)>>,
    errored: bool,
}

impl fmt::Debug for Copier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Copier")
            .field("active", &self.copier.is_some())
            .field("errored", &self.errored)
            .finish()
    }
}

/// # Safety
///
/// Copier will only be accessed by `&mut Self`.
unsafe impl Sync for Copier {}

impl Copier {
    /// Create a new copier.
    pub(crate) fn create(
        ctx: OperationContext,
        srv: Servicer,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self> {
        let copier = srv.copy(&ctx, from, to, args)?;

        Ok(Self {
            copier: Some(copier),
            metadata: None,

            fut: None,
            errored: false,
        })
    }

    /// Advance the copy operation by one step.
    pub async fn next(&mut self) -> Result<Option<usize>> {
        if self.errored {
            return Ok(None);
        }

        let Some(copier) = self.copier.as_mut() else {
            return Ok(None);
        };

        match copier.next_dyn().await {
            Ok(Some(n)) => Ok(Some(n)),
            Ok(None) => {
                self.metadata = Some(copier.close_dyn().await?);
                self.copier = None;
                Ok(None)
            }
            Err(err) => {
                self.errored = true;
                Err(err)
            }
        }
    }

    /// Abort the pending copy operation.
    pub async fn abort(&mut self) -> Result<()> {
        let Some(copier) = self.copier.as_mut() else {
            return Ok(());
        };

        copier.abort_dyn().await
    }

    /// Close the copier and return metadata from the server-side completion response.
    pub async fn close(&mut self) -> Result<Metadata> {
        if self.errored {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "copier has already failed and can't be closed",
            ));
        }

        if let Some(metadata) = self.metadata.clone() {
            return Ok(metadata);
        }

        while self.metadata.is_none() {
            match self.next().await? {
                Some(_) => continue,
                None => break,
            }
        }

        Ok(self
            .metadata
            .clone()
            .unwrap_or_else(|| MetadataBuilder::unknown().build()))
    }
}

impl Stream for Copier {
    type Item = Result<usize>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.errored {
            return Poll::Ready(None);
        }

        if let Some(mut copier) = self.copier.take() {
            let fut = async move {
                let res = copier.next_dyn().await;
                (copier, res)
            };
            self.fut = Some(Box::pin(fut));
        }

        if let Some(fut) = self.fut.as_mut() {
            let (copier, res) = ready!(fut.as_mut().poll(cx));
            self.copier = Some(copier);
            self.fut = None;

            return match res {
                Ok(Some(n)) => Poll::Ready(Some(Ok(n))),
                Ok(None) => Poll::Ready(None),
                Err(err) => {
                    self.errored = true;
                    Poll::Ready(Some(Err(err)))
                }
            };
        }

        Poll::Ready(None)
    }
}
