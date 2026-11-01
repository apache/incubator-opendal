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
use std::fmt::Formatter;
use std::sync::Arc;

use crate::raw::oio;
use crate::raw::*;
use crate::*;

/// CompleteLayer keeps validation wrappers for read/write operations.
pub struct CompleteLayer;

impl std::fmt::Debug for CompleteLayer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompleteLayer").finish()
    }
}

impl Layer for CompleteLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl CompleteLayer {
    fn layer(&self, inner: Servicer) -> CompleteService {
        CompleteService { inner }
    }
}

/// Provide complete wrapper for backend.
pub struct CompleteService {
    inner: Servicer,
}

impl std::fmt::Debug for CompleteService {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl Service for CompleteService {
    type Reader = CompleteReader<oio::Reader>;
    type Writer = CompleteWriter<oio::Writer>;
    type Lister = CompleteLister;
    type Deleter = oio::Deleter;
    type Copier = CompleteCopier;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.inner.info()
    }

    fn capability(&self) -> Capability {
        self.inner.capability()
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.inner.create_dir(ctx, path, args).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let reader = self.inner.read(ctx, path, args)?;
        Ok(CompleteReader::new(reader))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let append = args.append();
        let w = self.inner.write(ctx, path, args)?;
        Ok(CompleteWriter::new(w, append))
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let source_content_length_hint = args.source_content_length_hint();
        let copier = self.inner.copy(ctx, from, to, args)?;
        Ok(CompleteCopier::new(copier, source_content_length_hint))
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.inner.compose(ctx, to, args)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.inner.rename(ctx, from, to, args).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.inner.restore(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let metadata = self.inner.stat(ctx, path, args).await?.into_metadata();
        Ok(RpStat::new(metadata))
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner.delete(ctx)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let lister = self.inner.list(ctx, path, args)?;
        Ok(CompleteLister::new(ctx.clone(), self.inner.clone(), lister))
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.inner.presign(ctx, path, args).await
    }
}

pub struct CompleteCopier {
    inner: oio::Copier,
    source_content_length_hint: Option<u64>,
    copied: Option<u64>,
}

impl CompleteCopier {
    fn new(inner: oio::Copier, source_content_length_hint: Option<u64>) -> Self {
        Self {
            inner,
            source_content_length_hint,
            copied: None,
        }
    }

    fn complete_metadata(&self, metadata: Metadata) -> Result<Metadata> {
        if metadata.is_file() {
            if let Some(copied) = self.copied
                && metadata.content_length() != copied
            {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "copy result content length does not match copied bytes",
                )
                .with_operation("CompleteCopier::close")
                .with_context("expected", copied)
                .with_context("actual", metadata.content_length()));
            }

            return Ok(metadata);
        }

        let Some(copied) = self.copied.or(self.source_content_length_hint) else {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "copy result does not contain enough information to determine content length",
            )
            .with_operation("CompleteCopier::close"));
        };
        let mut builder = metadata.into_builder();
        builder.set_file(copied);
        Ok(builder.build())
    }
}

impl oio::Copy for CompleteCopier {
    async fn next(&mut self) -> Result<Option<usize>> {
        let progress = self.inner.next().await?;
        if let Some(progress) = progress {
            let copied = self.copied.unwrap_or_default();
            self.copied = Some(copied.checked_add(progress as u64).ok_or_else(|| {
                Error::new(ErrorKind::Unexpected, "copied byte count overflowed u64")
                    .with_operation("CompleteCopier::next")
            })?);
        }
        Ok(progress)
    }

    async fn close(&mut self) -> Result<Metadata> {
        let metadata = self.inner.close().await?;
        self.complete_metadata(metadata)
    }

    async fn abort(&mut self) -> Result<()> {
        self.inner.abort().await
    }
}

pub struct CompleteLister {
    inner: oio::Lister,
    ctx: OperationContext,
    srv: Servicer,
}

impl CompleteLister {
    fn new(ctx: OperationContext, srv: Servicer, inner: oio::Lister) -> Self {
        Self { inner, ctx, srv }
    }

    async fn complete_unknown_entry_mode(&self, entry: oio::Entry) -> Result<oio::Entry> {
        let path = entry.path().to_string();
        let version = entry.metadata().version().map(str::to_owned);
        let op = options::StatOptions {
            version,
            ..Default::default()
        }
        .into();

        let stat_metadata = match self.srv.stat(&self.ctx, &path, op).await {
            Ok(rp) => rp.into_metadata(),
            // The entry comes from the service's own listing, so a stat that
            // cannot see its target — a dangling symlink in fs, whose stat
            // follows the link — cannot refine the mode, but it does not mean
            // the entry is gone. Keep the entry as listed.
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(entry),
            Err(err) => return Err(err),
        };
        if stat_metadata.mode() == EntryMode::Unknown {
            return Ok(entry);
        }

        let (path, metadata) = entry.into_parts();
        let mut builder = metadata.into_builder();
        if stat_metadata.is_file() {
            builder.set_file(stat_metadata.content_length());
        } else {
            builder.set_dir();
        }
        Ok(oio::Entry::with(path, builder.build()))
    }
}

impl oio::List for CompleteLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        let Some(entry) = self.inner.next().await? else {
            return Ok(None);
        };

        if entry.mode() != EntryMode::Unknown || entry.metadata().is_deleted() {
            return Ok(Some(entry));
        }

        Ok(Some(self.complete_unknown_entry_mode(entry).await?))
    }
}

pub struct CompleteReader<R> {
    inner: R,
}

impl<R> CompleteReader<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: oio::Read> oio::Read for CompleteReader<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let size = if range.is_suffix() {
            None
        } else {
            range.size()
        };
        let (rp, stream) = self.inner.open(range).await?;
        Ok((
            rp,
            Box::new(CompleteReadStream::new(stream, size)) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let size = if range.is_suffix() {
            None
        } else {
            range.size()
        };
        let (rp, buffer) = self.inner.read(range).await?;
        check_complete(size, buffer.len() as u64)?;
        Ok((rp, buffer))
    }
}

pub struct CompleteReadStream<R> {
    inner: R,
    size: Option<u64>,
    read: u64,
}

impl<R> CompleteReadStream<R> {
    pub fn new(inner: R, size: Option<u64>) -> Self {
        Self {
            inner,
            size,
            read: 0,
        }
    }

    pub fn check(&self) -> Result<()> {
        check_complete(self.size, self.read)
    }
}

impl<R: oio::ReadStream> oio::ReadStream for CompleteReadStream<R> {
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

fn check_complete(size: Option<u64>, actual: u64) -> Result<()> {
    let Some(size) = size else {
        return Ok(());
    };

    match actual.cmp(&size) {
        Ordering::Equal => Ok(()),
        Ordering::Less => Err(
            Error::new(ErrorKind::Unexpected, "reader got too little data")
                .with_context("expect", size)
                .with_context("actual", actual),
        ),
        Ordering::Greater => Err(
            Error::new(ErrorKind::Unexpected, "reader got too much data")
                .with_context("expect", size)
                .with_context("actual", actual),
        ),
    }
}

/// Tracks the state of the Write operation.
/// A successful operation goes through states: Open -> Written -> Closed
/// A failed operation terminates in the Error state
#[derive(Debug, PartialEq, Eq)]
enum CompleteState {
    Open,
    Written,
    Closed,
    Error,
}

impl CompleteState {
    /// Attempt to transition to the destination state. Once CompleteState has
    /// errored all further transitions are ignored.
    fn transition(&mut self, destination: CompleteState) {
        if *self != CompleteState::Error {
            *self = destination
        }
    }
}

pub struct CompleteWriter<W> {
    inner: Option<W>,
    append: bool,
    size: u64,
    state: CompleteState,
}

impl<W> CompleteWriter<W> {
    pub fn new(inner: W, append: bool) -> CompleteWriter<W> {
        CompleteWriter {
            inner: Some(inner),
            append,
            size: 0,
            state: CompleteState::Open,
        }
    }

    fn check(&self, content_length: u64) -> Result<()> {
        if self.append {
            return Ok(());
        }

        match self.size.cmp(&content_length) {
            Ordering::Equal => Ok(()),
            Ordering::Less => Err(
                Error::new(ErrorKind::Unexpected, "writer got too little data")
                    .with_context("expect", content_length)
                    .with_context("actual", self.size),
            ),
            Ordering::Greater => Err(
                Error::new(ErrorKind::Unexpected, "writer got too much data")
                    .with_context("expect", content_length)
                    .with_context("actual", self.size),
            ),
        }
    }
}

/// Check if the writer has been closed or aborted while debug_assertions
/// enabled. This code will never be executed in release mode.
#[cfg(debug_assertions)]
impl<W> Drop for CompleteWriter<W> {
    fn drop(&mut self) {
        if self.state == CompleteState::Written {
            log::warn!(
                "writer has not been closed or aborted after successful write operation, must be a bug"
            )
        }
    }
}

impl<W> oio::Write for CompleteWriter<W>
where
    W: oio::Write,
{
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let w = self.inner.as_mut().ok_or_else(|| {
            debug_assert_ne!(
                self.state,
                CompleteState::Open,
                "bug: inner is empty, but state is Open"
            );
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;

        let len = bs.len();
        w.write(bs)
            .await
            .inspect_err(|_| self.state.transition(CompleteState::Error))?;
        self.size += len as u64;
        self.state.transition(CompleteState::Written);

        Ok(())
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        let w = self.inner.as_mut().ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;
        let size = range.size().ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "native writer copy requires a bounded range",
            )
        })?;

        w.copy_from(path, args, range)
            .await
            .inspect_err(|_| self.state.transition(CompleteState::Error))?;
        self.size += size;
        self.state.transition(CompleteState::Written);
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let w = self.inner.as_mut().ok_or_else(|| {
            debug_assert_ne!(
                self.state,
                CompleteState::Open,
                "bug: inner is empty, but state is Open"
            );
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;

        // we must return `Err` before setting inner to None; otherwise,
        // we won't be able to retry `close` in `RetryLayer`.
        let ret = w
            .close()
            .await
            .inspect_err(|_| self.state.transition(CompleteState::Error))?;
        let ret = if ret.is_file() {
            self.check(ret.content_length())
                .inspect_err(|_| self.state.transition(CompleteState::Error))?;
            ret
        } else if self.append {
            let err = Error::new(
                ErrorKind::Unexpected,
                "append result does not contain the final content length",
            )
            .with_operation("CompleteWriter::close");
            self.state.transition(CompleteState::Error);
            return Err(err);
        } else {
            let mut builder = ret.into_builder();
            builder.set_file(self.size);
            builder.build()
        };
        self.inner = None;
        self.state.transition(CompleteState::Closed);

        Ok(ret)
    }

    async fn abort(&mut self) -> Result<()> {
        let w = self.inner.as_mut().ok_or_else(|| {
            Error::new(ErrorKind::Unexpected, "writer has been closed or aborted")
        })?;

        w.abort()
            .await
            .inspect_err(|_| self.state.transition(CompleteState::Error))?;
        self.inner = None;
        self.state.transition(CompleteState::Closed);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::raw::oio::Copy as _;
    use crate::raw::oio::Write as _;

    struct MockCopier {
        progress: VecDeque<usize>,
        metadata: Metadata,
    }

    impl MockCopier {
        fn new(progress: impl IntoIterator<Item = usize>, metadata: Metadata) -> Self {
            Self {
                progress: progress.into_iter().collect(),
                metadata,
            }
        }
    }

    impl oio::Copy for MockCopier {
        async fn next(&mut self) -> Result<Option<usize>> {
            Ok(self.progress.pop_front())
        }

        async fn close(&mut self) -> Result<Metadata> {
            Ok(self.metadata.clone())
        }

        async fn abort(&mut self) -> Result<()> {
            Ok(())
        }
    }

    fn incomplete_copy_metadata() -> Metadata {
        MetadataBuilder::unknown().build()
    }

    #[tokio::test]
    async fn test_copy_uses_source_content_length_hint() -> Result<()> {
        let inner = Box::new(MockCopier::new([], incomplete_copy_metadata()));
        let mut copier = CompleteCopier::new(inner, Some(8));

        let metadata = copier.close().await?;

        assert!(metadata.is_file());
        assert_eq!(metadata.content_length(), 8);
        Ok(())
    }

    #[tokio::test]
    async fn test_copy_uses_reported_progress() -> Result<()> {
        let inner = Box::new(MockCopier::new([3, 5], incomplete_copy_metadata()));
        let mut copier = CompleteCopier::new(inner, Some(7));

        while copier.next().await?.is_some() {}
        let metadata = copier.close().await?;

        assert!(metadata.is_file());
        assert_eq!(metadata.content_length(), 8);
        Ok(())
    }

    #[tokio::test]
    async fn test_copy_uses_result_metadata_over_source_hint() -> Result<()> {
        let metadata = MetadataBuilder::file(8);
        let inner = Box::new(MockCopier::new([], metadata.build()));
        let mut copier = CompleteCopier::new(inner, Some(7));

        let metadata = copier.close().await?;

        assert_eq!(metadata.content_length(), 8);
        Ok(())
    }

    #[tokio::test]
    async fn test_copy_requires_authoritative_content_length() {
        let inner = Box::new(MockCopier::new([], incomplete_copy_metadata()));
        let mut copier = CompleteCopier::new(inner, None);

        let err = copier
            .close()
            .await
            .expect_err("copy without an authoritative content length must fail");

        assert_eq!(err.kind(), ErrorKind::Unexpected);
    }

    struct UnsupportedCopyWriter;

    impl oio::Write for UnsupportedCopyWriter {
        async fn write(&mut self, _: Buffer) -> Result<()> {
            Ok(())
        }

        async fn copy_from(&mut self, _: &str, _: OpRead, _: BytesRange) -> Result<()> {
            Err(Error::new(ErrorKind::Unsupported, "copy is unsupported"))
        }

        async fn close(&mut self) -> Result<Metadata> {
            Ok(MetadataBuilder::unknown().build())
        }

        async fn abort(&mut self) -> Result<()> {
            Ok(())
        }
    }

    struct MockWriter {
        metadata: Metadata,
    }

    impl oio::Write for MockWriter {
        async fn write(&mut self, _: Buffer) -> Result<()> {
            Ok(())
        }

        async fn close(&mut self) -> Result<Metadata> {
            Ok(self.metadata.clone())
        }

        async fn abort(&mut self) -> Result<()> {
            Ok(())
        }
    }

    struct MockReadReader {
        buffer: Buffer,
    }

    impl oio::Read for MockReadReader {
        async fn open(&self, _: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
            Err(Error::new(ErrorKind::Unsupported, "open is not supported"))
        }

        async fn read(&self, _: BytesRange) -> Result<(RpRead, Buffer)> {
            Ok((
                RpRead::new(MetadataBuilder::file(self.buffer.len() as u64).build()),
                self.buffer.clone(),
            ))
        }
    }

    fn new_test_reader(buffer: impl Into<Buffer>) -> CompleteReader<MockReadReader> {
        CompleteReader::new(MockReadReader {
            buffer: buffer.into(),
        })
    }

    #[tokio::test]
    async fn test_read_rejects_short_buffer() {
        let reader = new_test_reader("a");

        let err = oio::Read::read(&reader, BytesRange::from(0_u64..2))
            .await
            .expect_err("read should reject short buffer");

        assert_eq!(err.kind(), ErrorKind::Unexpected);
    }

    #[tokio::test]
    async fn test_read_rejects_extra_buffer() {
        let reader = new_test_reader("ab");

        let err = oio::Read::read(&reader, BytesRange::from(0_u64..1))
            .await
            .expect_err("read should reject extra buffer");

        assert_eq!(err.kind(), ErrorKind::Unexpected);
    }

    #[tokio::test]
    async fn test_writer_copy_from_error_is_terminal() {
        let mut writer = CompleteWriter::new(UnsupportedCopyWriter, false);

        let err = writer
            .copy_from("source", OpRead::new(), BytesRange::new(0, Some(8)))
            .await
            .expect_err("copy_from should fail");
        assert_eq!(err.kind(), ErrorKind::Unsupported);
        assert_eq!(writer.state, CompleteState::Error);

        writer.abort().await.unwrap();
        assert!(writer.inner.is_none());
    }

    #[tokio::test]
    async fn test_writer_promotes_unknown_metadata_with_written_size() -> Result<()> {
        let inner = MockWriter {
            metadata: MetadataBuilder::unknown().build(),
        };
        let mut writer = CompleteWriter::new(inner, false);
        writer.write(Buffer::from("abc")).await?;

        let metadata = writer.close().await?;
        assert!(metadata.is_file());
        assert_eq!(metadata.content_length(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_writer_preserves_explicit_empty_file_length() -> Result<()> {
        let inner = MockWriter {
            metadata: MetadataBuilder::file(0).build(),
        };
        let mut writer = CompleteWriter::new(inner, false);

        let metadata = writer.close().await?;
        assert!(metadata.is_file());
        assert_eq!(metadata.content_length(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_writer_rejects_explicit_zero_after_writing() {
        let inner = MockWriter {
            metadata: MetadataBuilder::file(0).build(),
        };
        let mut writer = CompleteWriter::new(inner, false);
        writer.write(Buffer::from("a")).await.unwrap();

        let err = writer
            .close()
            .await
            .expect_err("an explicit empty-file result must not mean missing length");
        assert_eq!(err.kind(), ErrorKind::Unexpected);
    }

    #[tokio::test]
    async fn test_append_requires_final_content_length() {
        let inner = MockWriter {
            metadata: MetadataBuilder::unknown().build(),
        };
        let mut writer = CompleteWriter::new(inner, true);
        writer.write(Buffer::from("a")).await.unwrap();

        let err = writer
            .close()
            .await
            .expect_err("append without the final content length must fail");
        assert_eq!(err.kind(), ErrorKind::Unexpected);
    }
}
