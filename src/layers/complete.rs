// Copyright 2023 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::io;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;

use crate::ops::*;
use crate::raw::output::into_reader::RangeReader;
use crate::raw::output::to_flat_pager;
use crate::raw::output::to_hierarchy_pager;
use crate::raw::output::Entry;
use crate::raw::output::IntoStreamableReader;
use crate::raw::output::ToFlatPager;
use crate::raw::output::ToHierarchyPager;
use crate::raw::*;
use crate::*;

/// Complete underlying services features so that users can use them in
/// the same way.
///
/// So far CompleteLayer will do two completion:
///
/// ## Read
///
/// OpenDAL requires all reader implements [`output::Read`] and
/// [`output::BlockingRead`]. However, not all services have the
/// capabilities. CompleteLayer will add those capabilities in
/// a zero cost way.
///
/// Underlying services will return [`AccessorHint`] to indicate the
/// features that returning readers support.
///
/// - If both `seekable` and `streamable`, return directly.
/// - If not `streamable`, with [`output::into_streamable_reader`].
/// - If not `seekable`, with [`output::into_reader::by_range`]
/// - If neither not supported, wrap both by_range and into_streamable.
///
/// ## List
///
/// There are two styles of list, but not all services support both of
/// them. CompleteLayer will add those capabilities in a zero cost way.
///
/// Underlying services will return [`AccessorHint`] to indicate the
/// features that returning pagers support.
///
/// - If both `flat` and `hierarchy`, return directly.
/// - If only `flat`, with [`output::to_flat_pager`].
/// - if only `hierarchy`, with [`output::to_hierarchy_pager`].
/// - If neither not supported, something must be wrong.
///
/// [`AccessorHint`]: crate::raw::AccessorHint
pub struct CompleteLayer;

impl<A: Accessor> Layer<A> for CompleteLayer {
    type LayeredAccessor = CompleteReaderAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccessor {
        let meta = inner.metadata();
        CompleteReaderAccessor {
            meta,
            inner: Arc::new(inner),
        }
    }
}

/// Provide reader wrapper for backend.
pub struct CompleteReaderAccessor<A: Accessor> {
    meta: AccessorMetadata,
    inner: Arc<A>,
}

impl<A: Accessor> Debug for CompleteReaderAccessor<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<A: Accessor> CompleteReaderAccessor<A> {
    async fn complete_reader(
        &self,
        path: &str,
        args: OpRead,
    ) -> Result<(RpRead, CompleteReader<A, A::Reader>)> {
        let (seekable, streamable) = (
            self.meta.hints().contains(AccessorHint::ReadSeekable),
            self.meta.hints().contains(AccessorHint::ReadStreamable),
        );

        let range = args.range();
        let (rp, r) = self.inner.read(path, args).await?;
        let content_length = rp.metadata().content_length();

        match (seekable, streamable) {
            (true, true) => Ok((rp, CompleteReader::AlreadyComplete(r))),
            (true, false) => {
                let r = output::into_streamable_reader(r, 256 * 1024);
                Ok((rp, CompleteReader::NeedStreamable(r)))
            }
            _ => {
                let (offset, size) = match (range.offset(), range.size()) {
                    (Some(offset), _) => (offset, content_length),
                    (None, None) => (0, content_length),
                    (None, Some(size)) => {
                        // TODO: we can read content range to calculate
                        // the total content length.
                        let om = self.inner.stat(path, OpStat::new()).await?.into_metadata();
                        let total_size = om.content_length();
                        let (offset, size) = if size > total_size {
                            (0, total_size)
                        } else {
                            (total_size - size, size)
                        };

                        (offset, size)
                    }
                };
                let r = output::into_reader::by_range(self.inner.clone(), path, r, offset, size);

                if streamable {
                    Ok((rp, CompleteReader::NeedSeekable(r)))
                } else {
                    let r = output::into_streamable_reader(r, 256 * 1024);
                    Ok((rp, CompleteReader::NeedBoth(r)))
                }
            }
        }
    }

    fn complete_blocking_reader(
        &self,
        path: &str,
        args: OpRead,
    ) -> Result<(RpRead, CompleteReader<A, A::BlockingReader>)> {
        let (seekable, streamable) = (
            self.meta.hints().contains(AccessorHint::ReadSeekable),
            self.meta.hints().contains(AccessorHint::ReadStreamable),
        );

        let (rp, r) = self.inner.blocking_read(path, args)?;

        match (seekable, streamable) {
            (true, true) => Ok((rp, CompleteReader::AlreadyComplete(r))),
            (true, false) => {
                let r = output::into_streamable_reader(r, 256 * 1024);
                Ok((rp, CompleteReader::NeedStreamable(r)))
            }
            (false, _) => Err(Error::new(
                ErrorKind::Unsupported,
                "non seekable blocking reader is not supported",
            )),
        }
    }

    async fn complete_list(
        &self,
        path: &str,
        args: OpList,
    ) -> Result<(RpList, CompletePager<A, A::Pager>)> {
        let (can_list, can_scan) = (
            self.meta.capabilities().contains(AccessorCapability::List),
            self.meta.capabilities().contains(AccessorCapability::Scan),
        );

        if can_list {
            let (rp, p) = self.inner.list(path, args).await?;
            Ok((rp, CompletePager::AlreadyComplete(p)))
        } else if can_scan {
            let (_, p) = self.inner.scan(path, OpScan::new()).await?;
            let p = to_hierarchy_pager(p, path);
            Ok((RpList::default(), CompletePager::NeedHierarchy(p)))
        } else {
            Err(
                Error::new(ErrorKind::Unsupported, "operation is not supported")
                    .with_context("service", self.meta.scheme())
                    .with_operation("list"),
            )
        }
    }

    fn complete_blocking_list(
        &self,
        path: &str,
        args: OpList,
    ) -> Result<(RpList, CompletePager<A, A::BlockingPager>)> {
        let (can_list, can_scan) = (
            self.meta.capabilities().contains(AccessorCapability::List),
            self.meta.capabilities().contains(AccessorCapability::Scan),
        );

        if can_list {
            let (rp, p) = self.inner.blocking_list(path, args)?;
            Ok((rp, CompletePager::AlreadyComplete(p)))
        } else if can_scan {
            let (_, p) = self.inner.blocking_scan(path, OpScan::new())?;
            let p = to_hierarchy_pager(p, path);
            Ok((RpList::default(), CompletePager::NeedHierarchy(p)))
        } else {
            Err(
                Error::new(ErrorKind::Unsupported, "operation is not supported")
                    .with_context("service", self.meta.scheme())
                    .with_operation("list"),
            )
        }
    }

    async fn complete_scan(
        &self,
        path: &str,
        args: OpScan,
    ) -> Result<(RpScan, CompletePager<A, A::Pager>)> {
        let (can_list, can_scan) = (
            self.meta.capabilities().contains(AccessorCapability::List),
            self.meta.capabilities().contains(AccessorCapability::Scan),
        );

        if can_scan {
            let (rp, p) = self.inner.scan(path, args).await?;
            Ok((rp, CompletePager::AlreadyComplete(p)))
        } else if can_list {
            let p = to_flat_pager(self.inner.clone(), path, args.limit().unwrap_or(1000));
            Ok((RpScan::default(), CompletePager::NeedFlat(p)))
        } else {
            Err(
                Error::new(ErrorKind::Unsupported, "operation is not supported")
                    .with_context("service", self.meta.scheme())
                    .with_operation("scan"),
            )
        }
    }

    fn complete_blocking_scan(
        &self,
        path: &str,
        args: OpScan,
    ) -> Result<(RpScan, CompletePager<A, A::BlockingPager>)> {
        let (can_list, can_scan) = (
            self.meta.capabilities().contains(AccessorCapability::List),
            self.meta.capabilities().contains(AccessorCapability::Scan),
        );

        if can_scan {
            let (rp, p) = self.inner.blocking_scan(path, args)?;
            Ok((rp, CompletePager::AlreadyComplete(p)))
        } else if can_list {
            let p = to_flat_pager(self.inner.clone(), path, args.limit().unwrap_or(1000));
            Ok((RpScan::default(), CompletePager::NeedFlat(p)))
        } else {
            Err(
                Error::new(ErrorKind::Unsupported, "operation is not supported")
                    .with_context("service", self.meta.scheme())
                    .with_operation("scan"),
            )
        }
    }
}

#[async_trait]
impl<A: Accessor> LayeredAccessor for CompleteReaderAccessor<A> {
    type Inner = A;
    type Reader = CompleteReader<A, A::Reader>;
    type BlockingReader = CompleteReader<A, A::BlockingReader>;
    type Pager = CompletePager<A, A::Pager>;
    type BlockingPager = CompletePager<A, A::BlockingPager>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        self.complete_reader(path, args).await
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        self.complete_blocking_reader(path, args)
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(path, args).await.map(|v| {
            v.map_metadata(|m| {
                let bit = m.bit();
                m.with_bit(bit | ObjectMetakey::Complete)
            })
        })
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.blocking_stat(path, args).map(|v| {
            v.map_metadata(|m| {
                let bit = m.bit();
                m.with_bit(bit | ObjectMetakey::Complete)
            })
        })
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Pager)> {
        self.complete_list(path, args).await
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingPager)> {
        self.complete_blocking_list(path, args)
    }

    async fn scan(&self, path: &str, args: OpScan) -> Result<(RpScan, Self::Pager)> {
        self.complete_scan(path, args).await
    }

    fn blocking_scan(&self, path: &str, args: OpScan) -> Result<(RpScan, Self::BlockingPager)> {
        self.complete_blocking_scan(path, args)
    }
}

pub enum CompleteReader<A: Accessor, R> {
    AlreadyComplete(R),
    NeedSeekable(RangeReader<A>),
    NeedStreamable(IntoStreamableReader<R>),
    NeedBoth(IntoStreamableReader<RangeReader<A>>),
}

impl<A, R> output::Read for CompleteReader<A, R>
where
    A: Accessor<Reader = R>,
    R: output::Read,
{
    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        use CompleteReader::*;

        match self {
            AlreadyComplete(r) => r.poll_read(cx, buf),
            NeedSeekable(r) => r.poll_read(cx, buf),
            NeedStreamable(r) => r.poll_read(cx, buf),
            NeedBoth(r) => r.poll_read(cx, buf),
        }
    }

    fn poll_seek(&mut self, cx: &mut Context<'_>, pos: io::SeekFrom) -> Poll<io::Result<u64>> {
        use CompleteReader::*;

        match self {
            AlreadyComplete(r) => r.poll_seek(cx, pos),
            NeedSeekable(r) => r.poll_seek(cx, pos),
            NeedStreamable(r) => r.poll_seek(cx, pos),
            NeedBoth(r) => r.poll_seek(cx, pos),
        }
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<io::Result<bytes::Bytes>>> {
        use CompleteReader::*;

        match self {
            AlreadyComplete(r) => r.poll_next(cx),
            NeedSeekable(r) => r.poll_next(cx),
            NeedStreamable(r) => r.poll_next(cx),
            NeedBoth(r) => r.poll_next(cx),
        }
    }
}

impl<A, R> output::BlockingRead for CompleteReader<A, R>
where
    A: Accessor<BlockingReader = R>,
    R: output::BlockingRead,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use CompleteReader::*;

        match self {
            AlreadyComplete(r) => r.read(buf),
            NeedStreamable(r) => r.read(buf),
            _ => unreachable!("not supported types of complete reader"),
        }
    }

    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        use CompleteReader::*;

        match self {
            AlreadyComplete(r) => r.seek(pos),
            NeedStreamable(r) => r.seek(pos),
            _ => unreachable!("not supported types of complete reader"),
        }
    }

    fn next(&mut self) -> Option<io::Result<bytes::Bytes>> {
        use CompleteReader::*;

        match self {
            AlreadyComplete(r) => r.next(),
            NeedStreamable(r) => r.next(),
            _ => unreachable!("not supported types of complete reader"),
        }
    }
}

pub enum CompletePager<A: Accessor, P> {
    AlreadyComplete(P),
    NeedFlat(ToFlatPager<Arc<A>, P>),
    NeedHierarchy(ToHierarchyPager<P>),
}

#[async_trait]
impl<A, P> output::Page for CompletePager<A, P>
where
    A: Accessor<Pager = P>,
    P: output::Page,
{
    async fn next_page(&mut self) -> Result<Option<Vec<Entry>>> {
        use CompletePager::*;

        match self {
            AlreadyComplete(p) => p.next_page().await,
            NeedFlat(p) => p.next_page().await,
            NeedHierarchy(p) => p.next_page().await,
        }
    }
}

impl<A, P> output::BlockingPage for CompletePager<A, P>
where
    A: Accessor<BlockingPager = P>,
    P: output::BlockingPage,
{
    fn next_page(&mut self) -> Result<Option<Vec<Entry>>> {
        use CompletePager::*;

        match self {
            AlreadyComplete(p) => p.next_page(),
            NeedFlat(p) => p.next_page(),
            NeedHierarchy(p) => p.next_page(),
        }
    }
}
