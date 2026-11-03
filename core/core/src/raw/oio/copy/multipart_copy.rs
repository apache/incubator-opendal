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

use std::future::Future;
use std::sync::Arc;

use futures::FutureExt;
use futures::select;

use crate::raw::oio::MultipartPart;
use crate::raw::*;
use crate::*;

/// MultipartCopy is used to implement [`oio::Copy`] based on multipart copy.
///
/// By implementing MultipartCopy, services only need to provide the
/// service-specific multipart copy operations. [`MultipartCopier`] will drive
/// the upload id, part queue, completion, and abort state.
pub trait MultipartCopy: Send + Sync + Unpin + 'static {
    /// source_metadata returns source metadata for planning multipart copy.
    ///
    /// MultipartCopier will call this API when source content length hint is not
    /// provided.
    fn source_metadata(&self) -> impl Future<Output = Result<Metadata>> + MaybeSend;

    /// copy_once is used to copy the source object at once.
    ///
    /// MultipartCopier will call this API when the source object can be copied
    /// without starting a multipart copy.
    fn copy_once(&self) -> impl Future<Output = Result<Metadata>> + MaybeSend;

    /// initiate_copy starts a multipart copy and returns the upload id.
    fn initiate_copy(&self) -> impl Future<Output = Result<String>> + MaybeSend;

    /// copy_part copies one source range into one multipart upload part.
    ///
    /// - part_number is the index of the part, starting from 0.
    fn copy_part(
        &self,
        upload_id: &str,
        part_number: usize,
        range: BytesRange,
    ) -> impl Future<Output = Result<MultipartPart>> + MaybeSend;

    /// complete_copy completes the multipart copy with the ordered part list.
    fn complete_copy(
        &self,
        upload_id: &str,
        parts: &[MultipartPart],
    ) -> impl Future<Output = Result<Metadata>> + MaybeSend;

    /// abort_copy cancels the pending multipart copy and purges intermediate state.
    fn abort_copy(&self, upload_id: &str) -> impl Future<Output = Result<()>> + MaybeSend;
}

struct CopyInput<C: MultipartCopy> {
    copier: Arc<C>,
    executor: Executor,
    upload_id: Arc<String>,
    part_number: usize,
    range: BytesRange,
}

/// Build executor and capability inputs for [`MultipartCopier`].
pub trait IntoMultipartCopyContext {
    /// Convert into executor and capability.
    fn into_multipart_copy_context(self) -> (Executor, Capability);
}

impl IntoMultipartCopyContext for Executor {
    fn into_multipart_copy_context(self) -> (Executor, Capability) {
        (self, Capability::default())
    }
}

impl IntoMultipartCopyContext for (Executor, Capability) {
    fn into_multipart_copy_context(self) -> (Executor, Capability) {
        self
    }
}

impl<C: MultipartCopy> Clone for CopyInput<C> {
    fn clone(&self) -> Self {
        Self {
            copier: self.copier.clone(),
            executor: self.executor.clone(),
            upload_id: self.upload_id.clone(),
            part_number: self.part_number,
            range: self.range,
        }
    }
}

struct CopiedPart {
    part: MultipartPart,
    size: u64,
}

/// MultipartCopier implements [`oio::Copy`] based on multipart copy.
pub struct MultipartCopier<C: MultipartCopy> {
    copier: Arc<C>,
    executor: Executor,
    capability: Capability,

    upload_id: Option<Arc<String>>,
    parts: Vec<MultipartPart>,
    next_part_number: usize,
    next_offset: u64,
    source_size: Option<u64>,
    copy_once_threshold: u64,
    part_size: u64,
    concurrent: usize,
    completed: bool,
    metadata: Option<Metadata>,

    tasks: ConcurrentTasks<CopyInput<C>, CopiedPart>,
}

impl<C: MultipartCopy> MultipartCopier<C> {
    /// Create a new MultipartCopier.
    pub fn new(
        context: impl IntoMultipartCopyContext,
        inner: C,
        source_content_length_hint: Option<u64>,
        copy_once_threshold: u64,
        part_size: u64,
        concurrent: usize,
    ) -> Self {
        let (executor, capability) = context.into_multipart_copy_context();
        let copier = Arc::new(inner);
        let concurrent = concurrent.max(1);

        Self {
            copier,
            executor: executor.clone(),
            capability,
            upload_id: None,
            parts: Vec::new(),
            next_part_number: 0,
            next_offset: 0,
            source_size: source_content_length_hint,
            copy_once_threshold,
            part_size,
            concurrent,
            completed: false,
            metadata: None,

            tasks: ConcurrentTasks::new(executor, concurrent, 0, |input| {
                Box::pin(async move {
                    let size = input
                        .range
                        .size()
                        .expect("multipart copy range must be sized");
                    let fut =
                        input
                            .copier
                            .copy_part(&input.upload_id, input.part_number, input.range);

                    let result = match input.executor.timeout() {
                        None => fut.await.map(|part| CopiedPart { part, size }),
                        Some(timeout) => {
                            select! {
                                result = fut.fuse() => {
                                    result.map(|part| CopiedPart { part, size })
                                }
                                _ = timeout.fuse() => {
                                    Err(Error::new(
                                        ErrorKind::Unexpected, "copy part timeout")
                                            .with_context("upload_id", input.upload_id.to_string())
                                            .with_context("part_number", input.part_number.to_string())
                                            .set_temporary())
                                }
                            }
                        }
                    };

                    (input, result)
                })
            }),
        }
    }

    async fn source_size(&mut self) -> Result<u64> {
        match self.source_size {
            Some(size) => Ok(size),
            None => {
                let size = self.copier.source_metadata().await?.content_length();
                self.source_size = Some(size);
                Ok(size)
            }
        }
    }

    /// Ensure the planned multipart copy does not exceed the service's derived part-count limit.
    ///
    /// This is called before `initiate_copy` so we fail a copy operation before we make any IO.
    fn validate_part_count(&self, source_size: u64) -> Result<()> {
        let capability = self.capability;
        let (Some(max_total_size), Some(max_part_size)) = (
            capability.write_total_max_size,
            capability.write_multi_max_size,
        ) else {
            return Ok(());
        };

        if max_part_size == 0 {
            return Ok(());
        }

        let max_parts = (max_total_size as u64).div_ceil(max_part_size as u64);
        let part_count = source_size.div_ceil(self.part_size);
        if part_count > max_parts {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "multipart copy part count exceeds service limit, please increase `OpCopy::chunk`",
            )
            .with_context("source_size", source_size)
            .with_context("part_size", self.part_size)
            .with_context("part_count", part_count)
            .with_context("max_parts", max_parts));
        }

        Ok(())
    }

    async fn upload_id(&mut self) -> Result<Arc<String>> {
        match self.upload_id.clone() {
            Some(upload_id) => Ok(upload_id),
            None => {
                let upload_id = self.copier.initiate_copy().await?;
                let upload_id = Arc::new(upload_id);
                self.upload_id = Some(upload_id.clone());
                Ok(upload_id)
            }
        }
    }

    async fn fill_tasks(&mut self, upload_id: Arc<String>, source_size: u64) -> Result<()> {
        let mut scheduled = 0;
        let executor = self.executor.clone();

        while self.next_offset < source_size
            && self.tasks.has_remaining()
            && scheduled < self.concurrent
        {
            let size = self.part_size.min(source_size - self.next_offset);
            let range = BytesRange::new(self.next_offset, Some(size));

            let input = CopyInput {
                copier: self.copier.clone(),
                executor: executor.clone(),
                upload_id: upload_id.clone(),
                part_number: self.next_part_number,
                range,
            };

            loop {
                match self.tasks.execute(input.clone()).await {
                    Ok(()) => break,
                    Err(err) if err.is_temporary() => continue,
                    Err(err) => return Err(err),
                }
            }

            self.next_offset += size;
            self.next_part_number += 1;
            scheduled += 1;

            if self.tasks.has_result() {
                break;
            }
        }

        Ok(())
    }
}

impl<C> oio::Copy for MultipartCopier<C>
where
    C: MultipartCopy,
{
    async fn next(&mut self) -> Result<Option<usize>> {
        if self.completed {
            return Ok(None);
        }

        let source_size = self.source_size().await?;

        if self.upload_id.is_none() && source_size <= self.copy_once_threshold {
            self.metadata = Some(self.copier.copy_once().await?);
            self.completed = true;
            return Ok(None);
        }

        // Validate part count before performing any copy requests.
        if self.upload_id.is_none() {
            self.validate_part_count(source_size)?;
        }

        let upload_id = self.upload_id().await?;
        self.fill_tasks(upload_id.clone(), source_size).await?;

        loop {
            match self.tasks.next().await {
                Some(Ok(result)) => {
                    let size = result.size.try_into().map_err(|_| {
                        Error::new(
                            ErrorKind::Unexpected,
                            "multipart copy part size exceeds usize",
                        )
                    })?;
                    self.parts.push(result.part);
                    return Ok(Some(size));
                }
                Some(Err(err)) if err.is_temporary() => continue,
                Some(Err(err)) => return Err(err),
                None => break,
            }
        }

        if self.parts.len() != self.next_part_number {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "multipart copy part numbers mismatch, please report bug to opendal",
            )
            .with_context("expected", self.next_part_number)
            .with_context("actual", self.parts.len())
            .with_context("upload_id", upload_id.to_string()));
        }

        self.parts.sort_by_key(|part| part.part_number);
        self.metadata = Some(self.copier.complete_copy(&upload_id, &self.parts).await?);
        self.completed = true;
        Ok(None)
    }

    async fn close(&mut self) -> Result<Metadata> {
        while !self.completed {
            self.next().await?;
        }

        let metadata = self
            .metadata
            .clone()
            .unwrap_or_else(|| MetadataBuilder::unknown().build());
        let Some(source_size) = self.source_size else {
            return Ok(metadata);
        };
        if metadata.is_file() {
            if metadata.content_length() != source_size {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "multipart copy result content length does not match source",
                )
                .with_context("expected", source_size)
                .with_context("actual", metadata.content_length()));
            }
            return Ok(metadata);
        }

        let mut builder = metadata.into_builder();
        builder.set_file(source_size);
        Ok(builder.build())
    }

    async fn abort(&mut self) -> Result<()> {
        self.tasks.clear();
        let Some(upload_id) = self.upload_id.take() else {
            return Ok(());
        };

        self.copier.abort_copy(&upload_id).await?;
        self.completed = true;
        self.metadata = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::raw::oio::Copy;

    struct TestCopy {
        source_size: u64,
        source_metadata_calls: AtomicUsize,
        copy_once_calls: AtomicUsize,
        initiate_copy_calls: AtomicUsize,
    }

    impl TestCopy {
        fn new(source_size: u64) -> Arc<Self> {
            Arc::new(Self {
                source_size,
                source_metadata_calls: AtomicUsize::new(0),
                copy_once_calls: AtomicUsize::new(0),
                initiate_copy_calls: AtomicUsize::new(0),
            })
        }
    }

    impl MultipartCopy for Arc<TestCopy> {
        async fn source_metadata(&self) -> Result<Metadata> {
            self.source_metadata_calls.fetch_add(1, Ordering::Relaxed);
            Ok({
                let mut metadata = MetadataBuilder::unknown();
                metadata.set_file(self.source_size);
                metadata.build()
            })
        }

        async fn copy_once(&self) -> Result<Metadata> {
            self.copy_once_calls.fetch_add(1, Ordering::Relaxed);
            Ok(MetadataBuilder::unknown().build())
        }

        async fn initiate_copy(&self) -> Result<String> {
            self.initiate_copy_calls.fetch_add(1, Ordering::Relaxed);
            Ok("upload_id".to_string())
        }

        async fn copy_part(
            &self,
            _: &str,
            part_number: usize,
            range: BytesRange,
        ) -> Result<MultipartPart> {
            Ok(MultipartPart {
                part_number,
                etag: "etag".to_string(),
                checksum: None,
                size: range.size(),
            })
        }

        async fn complete_copy(&self, _: &str, _: &[MultipartPart]) -> Result<Metadata> {
            Ok(MetadataBuilder::unknown().build())
        }

        async fn abort_copy(&self, _: &str) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_content_length_hint_skips_source_metadata() -> Result<()> {
        let inner = TestCopy::new(8);
        let mut copier = MultipartCopier::new(Executor::default(), inner.clone(), Some(8), 8, 8, 1);

        let metadata = copier.close().await?;
        assert_eq!(inner.source_metadata_calls.load(Ordering::Relaxed), 0);
        assert_eq!(inner.copy_once_calls.load(Ordering::Relaxed), 1);
        assert!(metadata.is_file());
        assert_eq!(metadata.content_length(), 8);
        Ok(())
    }

    #[tokio::test]
    async fn test_missing_content_length_hint_loads_source_metadata() -> Result<()> {
        let inner = TestCopy::new(8);
        let mut copier = MultipartCopier::new(Executor::default(), inner.clone(), None, 8, 8, 1);

        let metadata = copier.close().await?;
        assert_eq!(inner.source_metadata_calls.load(Ordering::Relaxed), 1);
        assert_eq!(inner.copy_once_calls.load(Ordering::Relaxed), 1);
        assert!(metadata.is_file());
        assert_eq!(metadata.content_length(), 8);
        Ok(())
    }

    #[tokio::test]
    async fn test_validate_part_count_rejects_before_initiate() -> Result<()> {
        let capability = Capability {
            write_total_max_size: Some(2),
            write_multi_max_size: Some(1),
            ..Default::default()
        };

        // source_size=10, part_size=1, copy_once_threshold=0 -> 10 parts needed, only 2 allowed.
        let inner = TestCopy::new(10);
        let mut copier = MultipartCopier::new(
            /*context=*/ (Executor::default(), capability),
            /*inner=*/ inner.clone(),
            /*source_content_length_hint=*/ Some(10),
            /*copy_once_threshold=*/ 0,
            /*part_size=*/ 1,
            /*concurrent=*/ 1,
        );

        let err = copier
            .next()
            .await
            .expect_err("part count should exceed max_parts");
        assert_eq!(err.kind(), ErrorKind::Unsupported);
        // Validation must reject before any multipart state is created on the server.
        assert_eq!(inner.copy_once_calls.load(Ordering::Relaxed), 0);
        assert_eq!(inner.initiate_copy_calls.load(Ordering::Relaxed), 0);
        Ok(())
    }
}
