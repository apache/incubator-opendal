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

use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::sync::Arc;

use crate::raw::oio::FlatLister;
use crate::raw::oio::PrefixLister;
use crate::raw::*;
use crate::*;

/// Complete underlying services features so that users can use them in
/// the same way.
///
/// # Notes
///
/// CompleteLayer is not a public accessible layer that can be used by
/// external users. OpenDAL will make sure every accessor will apply this
/// layer once and only once.
///
/// # Internal
///
/// So far `CompleteLayer` will do the following things:
///
/// ## Stat Completion
///
/// Not all services support stat dir natively, but we can simulate it via list.
///
/// ## Read Completion
///
/// OpenDAL requires all reader implements [`oio::Read`] and
/// [`oio::BlockingRead`]. However, not all services have the
/// capabilities. CompleteLayer will add those capabilities in
/// a zero cost way.
///
/// Underlying services will return [`AccessorInfo`] to indicate the
/// features that returning readers support.
///
/// - If both `seekable` and `streamable`, return directly.
/// - If not `streamable`, with [`oio::into_read_from_stream`].
/// - If not `seekable`, with [`oio::into_seekable_read_by_range`]
/// - If neither not supported, wrap both by_range and into_streamable.
///
/// All implementations of Reader should be `zero cost`. In our cases,
/// which means others must pay the same cost for the same feature provide
/// by us.
///
/// For examples, call `read` without `seek` should always act the same as
/// calling `read` on plain reader.
///
/// ### Read is Seekable
///
/// We use [`Capability`] to decide the most suitable implementations.
///
/// If [`Capability`] `read_can_seek` is true, we will open it with given args
/// directly. Otherwise, we will pick a seekable reader implementation based
/// on input range for it.
///
/// - `Some(offset), Some(size)` => `RangeReader`
/// - `Some(offset), None` and `None, None` => `OffsetReader`
/// - `None, Some(size)` => get the total size first to convert as `RangeReader`
///
/// No matter which reader we use, we will make sure the `read` operation
/// is zero cost.
///
/// ### Read is Streamable
///
/// We use internal `AccessorHint::ReadStreamable` to decide the most
/// suitable implementations.
///
/// If [`Capability`] `read_can_next` is true, we will use existing reader
/// directly. Otherwise, we will use transform this reader as a stream.
///
/// ## List Completion
///
/// There are two styles of list, but not all services support both of
/// them. CompleteLayer will add those capabilities in a zero cost way.
///
/// Underlying services will return [`Capability`] to indicate the
/// features that returning listers support.
///
/// - If support `list_with_recursive`, return directly.
/// - if not, wrap with [`FlatLister`].
///
/// ## Capability Check
///
/// Before performing any operations, `CompleteLayer` will first check
/// the operation against capability of the underlying service. If the
/// operation is not supported, an error will be returned directly.
pub struct CompleteLayer;

impl<A: Access> Layer<A> for CompleteLayer {
    type LayeredAccess = CompleteAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        let mut meta = inner.info().as_ref().clone();
        let cap = meta.full_capability_mut();

        if cap.list && cap.write_can_empty {
            cap.create_dir = true;
        }

        CompleteAccessor {
            meta: meta.into(),
            inner: Arc::new(inner),
        }
    }
}

/// Provide complete wrapper for backend.
pub struct CompleteAccessor<A: Access> {
    meta: Arc<AccessorInfo>,
    inner: Arc<A>,
}

impl<A: Access> Debug for CompleteAccessor<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl<A: Access> CompleteAccessor<A> {
    fn new_unsupported_error(&self, op: impl Into<&'static str>) -> Error {
        let scheme = self.meta.scheme();
        let op = op.into();
        Error::new(
            ErrorKind::Unsupported,
            format!("service {scheme} doesn't support operation {op}"),
        )
        .with_operation(op)
    }

    async fn complete_create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        let capability = self.meta.full_capability();
        if capability.create_dir {
            return self.inner().create_dir(path, args).await;
        }
        if capability.write_can_empty && capability.list {
            let (_, mut w) = self.inner.write(path, OpWrite::default()).await?;
            oio::Write::close(&mut w).await?;
            return Ok(RpCreateDir::default());
        }

        Err(self.new_unsupported_error(Operation::CreateDir))
    }

    fn complete_blocking_create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        let capability = self.meta.full_capability();
        if capability.create_dir && capability.blocking {
            return self.inner().blocking_create_dir(path, args);
        }
        if capability.write_can_empty && capability.list && capability.blocking {
            let (_, mut w) = self.inner.blocking_write(path, OpWrite::default())?;
            oio::BlockingWrite::close(&mut w)?;
            return Ok(RpCreateDir::default());
        }

        Err(self.new_unsupported_error(Operation::BlockingCreateDir))
    }

    async fn complete_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let capability = self.meta.full_capability();
        if !capability.stat {
            return Err(self.new_unsupported_error(Operation::Stat));
        }

        if path == "/" {
            return Ok(RpStat::new(Metadata::new(EntryMode::DIR)));
        }

        // Forward to inner if create_dir is supported.
        if path.ends_with('/') && capability.create_dir {
            let meta = self.inner.stat(path, args).await?.into_metadata();

            if meta.is_file() {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "stat expected a directory, but found a file",
                ));
            }

            return Ok(RpStat::new(Metadata::new(EntryMode::DIR)));
        }

        // Otherwise, we can simulate stat dir via `list`.
        if path.ends_with('/') && capability.list_with_recursive {
            let (_, mut l) = self
                .inner
                .list(path, OpList::default().with_recursive(true).with_limit(1))
                .await?;

            return if oio::List::next(&mut l).await?.is_some() {
                Ok(RpStat::new(Metadata::new(EntryMode::DIR)))
            } else {
                Err(Error::new(
                    ErrorKind::NotFound,
                    "the directory is not found",
                ))
            };
        }

        // Forward to underlying storage directly since we don't know how to handle stat dir.
        self.inner.stat(path, args).await.map(|v| {
            v.map_metadata(|m| {
                let bit = m.metakey();
                m.with_metakey(bit | Metakey::Complete)
            })
        })
    }

    fn complete_blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let capability = self.meta.full_capability();
        if !capability.stat {
            return Err(self.new_unsupported_error(Operation::Stat));
        }

        if path == "/" {
            return Ok(RpStat::new(Metadata::new(EntryMode::DIR)));
        }

        // Forward to inner if create dir is supported.
        if path.ends_with('/') && capability.create_dir {
            let meta = self.inner.blocking_stat(path, args)?.into_metadata();

            if meta.is_file() {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    "stat expected a directory, but found a file",
                ));
            }

            return Ok(RpStat::new(Metadata::new(EntryMode::DIR)));
        }

        // Otherwise, we can simulate stat a dir path via `list`.
        if path.ends_with('/') && capability.list_with_recursive {
            let (_, mut l) = self
                .inner
                .blocking_list(path, OpList::default().with_recursive(true).with_limit(1))?;

            return if oio::BlockingList::next(&mut l)?.is_some() {
                Ok(RpStat::new(Metadata::new(EntryMode::DIR)))
            } else {
                Err(Error::new(
                    ErrorKind::NotFound,
                    "the directory is not found",
                ))
            };
        }

        // Forward to underlying storage directly since we don't know how to handle stat dir.
        self.inner.blocking_stat(path, args).map(|v| {
            v.map_metadata(|m| {
                let bit = m.metakey();
                m.with_metakey(bit | Metakey::Complete)
            })
        })
    }

    async fn complete_list(
        &self,
        path: &str,
        args: OpList,
    ) -> Result<(RpList, CompleteLister<A, A::Lister>)> {
        let cap = self.meta.full_capability();
        if !cap.list {
            return Err(self.new_unsupported_error(Operation::List));
        }

        let recursive = args.recursive();

        match (recursive, cap.list_with_recursive) {
            // - If service can list_with_recursive, we can forward list to it directly.
            (_, true) => {
                let (rp, p) = self.inner.list(path, args).await?;
                Ok((rp, CompleteLister::One(p)))
            }
            // If recursive is true but service can't list_with_recursive
            (true, false) => {
                // Forward path that ends with /
                if path.ends_with('/') {
                    let p = FlatLister::new(self.inner.clone(), path);
                    Ok((RpList::default(), CompleteLister::Two(p)))
                } else {
                    let parent = get_parent(path);
                    let p = FlatLister::new(self.inner.clone(), parent);
                    let p = PrefixLister::new(p, path);
                    Ok((RpList::default(), CompleteLister::Four(p)))
                }
            }
            // If recursive and service doesn't support list_with_recursive, we need to handle
            // list prefix by ourselves.
            (false, false) => {
                // Forward path that ends with /
                if path.ends_with('/') {
                    let (rp, p) = self.inner.list(path, args).await?;
                    Ok((rp, CompleteLister::One(p)))
                } else {
                    let parent = get_parent(path);
                    let (rp, p) = self.inner.list(parent, args).await?;
                    let p = PrefixLister::new(p, path);
                    Ok((rp, CompleteLister::Three(p)))
                }
            }
        }
    }

    fn complete_blocking_list(
        &self,
        path: &str,
        args: OpList,
    ) -> Result<(RpList, CompleteLister<A, A::BlockingLister>)> {
        let cap = self.meta.full_capability();
        if !cap.list {
            return Err(self.new_unsupported_error(Operation::BlockingList));
        }

        let recursive = args.recursive();

        match (recursive, cap.list_with_recursive) {
            // - If service can list_with_recursive, we can forward list to it directly.
            (_, true) => {
                let (rp, p) = self.inner.blocking_list(path, args)?;
                Ok((rp, CompleteLister::One(p)))
            }
            // If recursive is true but service can't list_with_recursive
            (true, false) => {
                // Forward path that ends with /
                if path.ends_with('/') {
                    let p = FlatLister::new(self.inner.clone(), path);
                    Ok((RpList::default(), CompleteLister::Two(p)))
                } else {
                    let parent = get_parent(path);
                    let p = FlatLister::new(self.inner.clone(), parent);
                    let p = PrefixLister::new(p, path);
                    Ok((RpList::default(), CompleteLister::Four(p)))
                }
            }
            // If recursive and service doesn't support list_with_recursive, we need to handle
            // list prefix by ourselves.
            (false, false) => {
                // Forward path that ends with /
                if path.ends_with('/') {
                    let (rp, p) = self.inner.blocking_list(path, args)?;
                    Ok((rp, CompleteLister::One(p)))
                } else {
                    let parent = get_parent(path);
                    let (rp, p) = self.inner.blocking_list(parent, args)?;
                    let p = PrefixLister::new(p, path);
                    Ok((rp, CompleteLister::Three(p)))
                }
            }
        }
    }
}

impl<A: Access> LayeredAccess for CompleteAccessor<A> {
    type Inner = A;
    type Reader = CompleteReader<A::Reader>;
    type BlockingReader = CompleteReader<A::BlockingReader>;
    type Writer = CompleteWriter<A::Writer>;
    type BlockingWriter = CompleteWriter<A::BlockingWriter>;
    type Lister = CompleteLister<A, A::Lister>;
    type BlockingLister = CompleteLister<A, A::BlockingLister>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    fn metadata(&self) -> Arc<AccessorInfo> {
        self.meta.clone()
    }

    async fn create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        self.complete_create_dir(path, args).await
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let capability = self.meta.full_capability();
        if !capability.read {
            return Err(self.new_unsupported_error(Operation::Read));
        }

        let size = args.range().size();
        self.inner
            .read(path, args)
            .await
            .map(|(rp, r)| (rp, CompleteReader::new(r, size)))
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let capability = self.meta.full_capability();
        if !capability.write {
            return Err(self.new_unsupported_error(Operation::Write));
        }
        if args.append() && !capability.write_can_append {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!(
                    "service {} doesn't support operation write with append",
                    self.info().scheme()
                ),
            ));
        }

        let (rp, w) = self.inner.write(path, args.clone()).await?;
        let w = CompleteWriter::new(w);
        Ok((rp, w))
    }

    async fn copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        let capability = self.meta.full_capability();
        if !capability.copy {
            return Err(self.new_unsupported_error(Operation::Copy));
        }

        self.inner().copy(from, to, args).await
    }

    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        let capability = self.meta.full_capability();
        if !capability.rename {
            return Err(self.new_unsupported_error(Operation::Rename));
        }

        self.inner().rename(from, to, args).await
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.complete_stat(path, args).await
    }

    async fn delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        let capability = self.meta.full_capability();
        if !capability.delete {
            return Err(self.new_unsupported_error(Operation::Delete));
        }

        self.inner().delete(path, args).await
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        let capability = self.meta.full_capability();
        if !capability.list {
            return Err(self.new_unsupported_error(Operation::List));
        }

        self.complete_list(path, args).await
    }

    async fn batch(&self, args: OpBatch) -> Result<RpBatch> {
        let capability = self.meta.full_capability();
        if !capability.batch {
            return Err(self.new_unsupported_error(Operation::Batch));
        }

        self.inner().batch(args).await
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        let capability = self.meta.full_capability();
        if !capability.presign {
            return Err(self.new_unsupported_error(Operation::Presign));
        }

        self.inner.presign(path, args).await
    }

    fn blocking_create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        self.complete_blocking_create_dir(path, args)
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        let capability = self.meta.full_capability();
        if !capability.read || !capability.blocking {
            return Err(self.new_unsupported_error(Operation::Read));
        }

        let size = args.range().size();
        self.inner
            .blocking_read(path, args)
            .map(|(rp, r)| (rp, CompleteReader::new(r, size)))
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        let capability = self.meta.full_capability();
        if !capability.write || !capability.blocking {
            return Err(self.new_unsupported_error(Operation::BlockingWrite));
        }

        if args.append() && !capability.write_can_append {
            return Err(Error::new(
                ErrorKind::Unsupported,
                format!(
                    "service {} doesn't support operation write with append",
                    self.info().scheme()
                ),
            ));
        }

        self.inner
            .blocking_write(path, args)
            .map(|(rp, w)| (rp, CompleteWriter::new(w)))
    }

    fn blocking_copy(&self, from: &str, to: &str, args: OpCopy) -> Result<RpCopy> {
        let capability = self.meta.full_capability();
        if !capability.copy || !capability.blocking {
            return Err(self.new_unsupported_error(Operation::BlockingCopy));
        }

        self.inner().blocking_copy(from, to, args)
    }

    fn blocking_rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        let capability = self.meta.full_capability();
        if !capability.rename || !capability.blocking {
            return Err(self.new_unsupported_error(Operation::BlockingRename));
        }

        self.inner().blocking_rename(from, to, args)
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.complete_blocking_stat(path, args)
    }

    fn blocking_delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        let capability = self.meta.full_capability();
        if !capability.delete || !capability.blocking {
            return Err(self.new_unsupported_error(Operation::BlockingDelete));
        }

        self.inner().blocking_delete(path, args)
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        let capability = self.meta.full_capability();
        if !capability.list || !capability.blocking {
            return Err(self.new_unsupported_error(Operation::BlockingList));
        }

        self.complete_blocking_list(path, args)
    }
}

pub type CompleteLister<A, P> =
    FourWays<P, FlatLister<Arc<A>, P>, PrefixLister<P>, PrefixLister<FlatLister<Arc<A>, P>>>;

pub struct CompleteReader<R> {
    inner: R,
    size: Option<u64>,
    read: u64,
}

impl<R> CompleteReader<R> {
    pub fn new(inner: R, size: Option<u64>) -> Self {
        Self {
            inner,
            size,
            read: 0,
        }
    }

    pub fn check(&self) -> Result<()> {
        let Some(size) = self.size else {
            return Ok(());
        };

        match self.read.cmp(&size) {
            Ordering::Equal => Ok(()),
            Ordering::Less => Err(
                Error::new(ErrorKind::Unexpected, "reader got too little data")
                    .with_context("expect", size)
                    .with_context("actual", self.read),
            ),
            Ordering::Greater => Err(
                Error::new(ErrorKind::Unexpected, "reader got too much data")
                    .with_context("expect", size)
                    .with_context("actual", self.read),
            ),
        }
    }
}

impl<R: oio::Read> oio::Read for CompleteReader<R> {
    async fn read(&mut self) -> Result<Buffer> {
        let buf = self.inner.read().await?;

        if buf.is_empty() {
            self.check()?;
        } else {
            self.read += buf.len() as u64;
        }

        Ok(buf)
    }
}

impl<R: oio::BlockingRead> oio::BlockingRead for CompleteReader<R> {
    fn read(&mut self) -> Result<Buffer> {
        let buf = self.inner.read()?;

        if buf.is_empty() {
            self.check()?;
        } else {
            self.read += buf.len() as u64;
        }

        Ok(buf)
    }
}

pub struct CompleteWriter<W> {
    inner: Option<W>,
}

impl<W> CompleteWriter<W> {
    pub fn new(inner: W) -> CompleteWriter<W> {
        CompleteWriter { inner: Some(inner) }
    }
}

/// Check if the writer has been closed or aborted while debug_assertions
/// enabled. This code will never be executed in release mode.
#[cfg(debug_assertions)]
impl<W> Drop for CompleteWriter<W> {
    fn drop(&mut self) {
        if self.inner.is_some() {
            log::warn!("writer has not been closed or aborted, must be a bug")
        }
    }
}

impl<W> oio::Write for CompleteWriter<W>
where
    W: oio::Write,
{
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let w = self.inner.as_mut().ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;

        w.write(bs).await
    }

    async fn close(&mut self) -> Result<()> {
        let w = self.inner.as_mut().ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;

        w.close().await?;
        self.inner = None;

        Ok(())
    }

    async fn abort(&mut self) -> Result<()> {
        let w = self.inner.as_mut().ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;

        w.abort().await?;
        self.inner = None;

        Ok(())
    }
}

impl<W> oio::BlockingWrite for CompleteWriter<W>
where
    W: oio::BlockingWrite,
{
    fn write(&mut self, bs: Buffer) -> Result<()> {
        let w = self.inner.as_mut().ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;

        w.write(bs)
    }

    fn close(&mut self) -> Result<()> {
        let w = self.inner.as_mut().ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;

        w.close()?;
        self.inner = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use http::HeaderMap;
    use http::Method as HttpMethod;

    use super::*;

    #[derive(Debug)]
    struct MockService {
        capability: Capability,
    }

    impl Access for MockService {
        type Reader = oio::Reader;
        type Writer = oio::Writer;
        type Lister = oio::Lister;
        type BlockingReader = oio::BlockingReader;
        type BlockingWriter = oio::BlockingWriter;
        type BlockingLister = oio::BlockingLister;

        fn info(&self) -> Arc<AccessorInfo> {
            let mut info = AccessorInfo::default();
            info.set_native_capability(self.capability);

            info.into()
        }

        async fn create_dir(&self, _: &str, _: OpCreateDir) -> Result<RpCreateDir> {
            Ok(RpCreateDir {})
        }

        async fn stat(&self, _: &str, _: OpStat) -> Result<RpStat> {
            Ok(RpStat::new(Metadata::new(EntryMode::Unknown)))
        }

        async fn read(&self, _: &str, _: OpRead) -> Result<(RpRead, Self::Reader)> {
            Ok((RpRead::new(), Box::new(bytes::Bytes::new())))
        }

        async fn write(&self, _: &str, _: OpWrite) -> Result<(RpWrite, Self::Writer)> {
            Ok((RpWrite::new(), Box::new(())))
        }

        async fn delete(&self, _: &str, _: OpDelete) -> Result<RpDelete> {
            Ok(RpDelete {})
        }

        async fn list(&self, _: &str, _: OpList) -> Result<(RpList, Self::Lister)> {
            Ok((RpList {}, Box::new(())))
        }

        async fn copy(&self, _: &str, _: &str, _: OpCopy) -> Result<RpCopy> {
            Ok(RpCopy {})
        }

        async fn rename(&self, _: &str, _: &str, _: OpRename) -> Result<RpRename> {
            Ok(RpRename {})
        }

        async fn presign(&self, _: &str, _: OpPresign) -> Result<RpPresign> {
            Ok(RpPresign::new(PresignedRequest::new(
                HttpMethod::POST,
                "https://example.com/presign".parse().expect("should parse"),
                HeaderMap::new(),
            )))
        }
    }

    fn new_test_operator(capability: Capability) -> Operator {
        let srv = MockService { capability };

        Operator::from_inner(Arc::new(srv)).layer(CompleteLayer)
    }

    #[tokio::test]
    async fn test_read() {
        let op = new_test_operator(Capability::default());
        let res = op.read("path").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            read: true,
            stat: true,
            ..Default::default()
        });
        let res = op.read("path").await;
        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_stat() {
        let op = new_test_operator(Capability::default());
        let res = op.stat("path").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            stat: true,
            ..Default::default()
        });
        let res = op.stat("path").await;
        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_writer() {
        let op = new_test_operator(Capability::default());
        let bs: Vec<u8> = vec![];
        let res = op.write("path", bs).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            write: true,
            ..Default::default()
        });
        let res = op.writer("path").await;
        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_create_dir() {
        let op = new_test_operator(Capability::default());
        let res = op.create_dir("path/").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            create_dir: true,
            ..Default::default()
        });
        let res = op.create_dir("path/").await;
        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_delete() {
        let op = new_test_operator(Capability::default());
        let res = op.delete("path").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            delete: true,
            ..Default::default()
        });
        let res = op.delete("path").await;
        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_copy() {
        let op = new_test_operator(Capability::default());
        let res = op.copy("path_a", "path_b").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            copy: true,
            ..Default::default()
        });
        let res = op.copy("path_a", "path_b").await;
        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_rename() {
        let op = new_test_operator(Capability::default());
        let res = op.rename("path_a", "path_b").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            rename: true,
            ..Default::default()
        });
        let res = op.rename("path_a", "path_b").await;
        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_list() {
        let op = new_test_operator(Capability::default());
        let res = op.list("path/").await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            list: true,
            list_with_recursive: true,
            ..Default::default()
        });
        let res = op.list("path/").await;
        assert!(res.is_ok())
    }

    #[tokio::test]
    async fn test_presign() {
        let op = new_test_operator(Capability::default());
        let res = op.presign_read("path", Duration::from_secs(1)).await;
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().kind(), ErrorKind::Unsupported);

        let op = new_test_operator(Capability {
            presign: true,
            ..Default::default()
        });
        let res = op.presign_read("path", Duration::from_secs(1)).await;
        assert!(res.is_ok())
    }
}
