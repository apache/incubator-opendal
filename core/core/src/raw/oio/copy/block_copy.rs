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
use uuid::Uuid;

use crate::raw::*;
use crate::*;

/// BlockCopy is used to implement [`oio::Copy`] based on block copy.
///
/// By implementing BlockCopy, services only need to provide service-specific
/// block copy operations. [`BlockCopier`] will drive source metadata loading,
/// block id generation, block queue, completion, and abort state.
pub trait BlockCopy: Send + Sync + Unpin + 'static {
    /// source_metadata returns source metadata for planning block copy.
    ///
    /// BlockCopier will call this API when source content length hint is not
    /// provided.
    fn source_metadata(&self) -> impl Future<Output = Result<Metadata>> + MaybeSend;

    /// copy_once is used to copy the source object at once.
    ///
    /// BlockCopier will call this API when the source object can be copied
    /// without starting block copy.
    fn copy_once(&self) -> impl Future<Output = Result<Metadata>> + MaybeSend;

    /// copy_block copies one source range into one block.
    fn copy_block(
        &self,
        block_id: Uuid,
        range: BytesRange,
    ) -> impl Future<Output = Result<()>> + MaybeSend;

    /// complete_block completes the block copy with the ordered block id list.
    fn complete_block(
        &self,
        block_ids: Vec<Uuid>,
    ) -> impl Future<Output = Result<Metadata>> + MaybeSend;

    /// abort_block cancels the pending block copy and purges intermediate state.
    fn abort_block(&self, block_ids: Vec<Uuid>) -> impl Future<Output = Result<()>> + MaybeSend;
}

struct CopyInput<C: BlockCopy> {
    copier: Arc<C>,
    executor: Executor,
    block_id: Uuid,
    block_number: usize,
    range: BytesRange,
}

impl<C: BlockCopy> Clone for CopyInput<C> {
    fn clone(&self) -> Self {
        Self {
            copier: self.copier.clone(),
            executor: self.executor.clone(),
            block_id: self.block_id,
            block_number: self.block_number,
            range: self.range,
        }
    }
}

struct CopiedBlock {
    block_id: Uuid,
    block_number: usize,
    size: u64,
}

/// BlockCopier implements [`oio::Copy`] based on block copy.
pub struct BlockCopier<C: BlockCopy> {
    copier: Arc<C>,
    executor: Executor,

    block_ids: Vec<(usize, Uuid)>,
    scheduled_block_ids: Vec<Uuid>,
    next_block_number: usize,
    next_offset: u64,
    source_size: Option<u64>,
    copy_once_threshold: u64,
    block_size: u64,
    concurrent: usize,
    completed: bool,
    metadata: Option<Metadata>,

    tasks: ConcurrentTasks<CopyInput<C>, CopiedBlock>,
}

impl<C: BlockCopy> BlockCopier<C> {
    /// Create a new BlockCopier.
    pub fn new(
        executor: impl Into<Executor>,
        inner: C,
        source_content_length_hint: Option<u64>,
        copy_once_threshold: u64,
        block_size: u64,
        concurrent: usize,
    ) -> Self {
        let executor = executor.into();
        let copier = Arc::new(inner);
        let concurrent = concurrent.max(1);

        Self {
            copier,
            executor: executor.clone(),
            block_ids: Vec::new(),
            scheduled_block_ids: Vec::new(),
            next_block_number: 0,
            next_offset: 0,
            source_size: source_content_length_hint,
            copy_once_threshold,
            block_size,
            concurrent,
            completed: false,
            metadata: None,

            tasks: ConcurrentTasks::new(executor, concurrent, 0, |input| {
                Box::pin(async move {
                    let size = input.range.size().expect("block copy range must be sized");
                    let fut = input.copier.copy_block(input.block_id, input.range);

                    let result = match input.executor.timeout() {
                        None => fut.await.map(|_| CopiedBlock {
                            block_id: input.block_id,
                            block_number: input.block_number,
                            size,
                        }),
                        Some(timeout) => {
                            select! {
                                result = fut.fuse() => {
                                    result.map(|_| CopiedBlock {
                                        block_id: input.block_id,
                                        block_number: input.block_number,
                                        size,
                                    })
                                }
                                _ = timeout.fuse() => {
                                    Err(Error::new(
                                        ErrorKind::Unexpected, "copy block timeout")
                                            .with_context("block_id", input.block_id.to_string())
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

    async fn fill_tasks(&mut self, source_size: u64) -> Result<()> {
        let mut scheduled = 0;

        while self.next_offset < source_size
            && self.tasks.has_remaining()
            && scheduled < self.concurrent
        {
            let size = self.block_size.min(source_size - self.next_offset);
            let range = BytesRange::new(self.next_offset, Some(size));

            let input = CopyInput {
                copier: self.copier.clone(),
                executor: self.executor.clone(),
                block_id: Uuid::new_v4(),
                block_number: self.next_block_number,
                range,
            };

            loop {
                match self.tasks.execute(input.clone()).await {
                    Ok(()) => break,
                    Err(err) if err.is_temporary() => continue,
                    Err(err) => return Err(err),
                }
            }

            self.scheduled_block_ids.push(input.block_id);
            self.next_offset += size;
            self.next_block_number += 1;
            scheduled += 1;

            if self.tasks.has_result() {
                break;
            }
        }

        Ok(())
    }
}

impl<C> oio::Copy for BlockCopier<C>
where
    C: BlockCopy,
{
    async fn next(&mut self) -> Result<Option<usize>> {
        if self.completed {
            return Ok(None);
        }

        let source_size = self.source_size().await?;

        if self.block_ids.is_empty() && source_size <= self.copy_once_threshold {
            self.metadata = Some(self.copier.copy_once().await?);
            self.completed = true;
            return Ok(None);
        }

        self.fill_tasks(source_size).await?;

        loop {
            match self.tasks.next().await {
                Some(Ok(result)) => {
                    let size = result.size.try_into().map_err(|_| {
                        Error::new(ErrorKind::Unexpected, "block copy size exceeds usize")
                    })?;
                    self.block_ids.push((result.block_number, result.block_id));
                    return Ok(Some(size));
                }
                Some(Err(err)) if err.is_temporary() => continue,
                Some(Err(err)) => return Err(err),
                None => break,
            }
        }

        if self.block_ids.len() != self.next_block_number {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "block copy numbers mismatch, please report bug to opendal",
            )
            .with_context("expected", self.next_block_number)
            .with_context("actual", self.block_ids.len()));
        }

        self.block_ids
            .sort_by_key(|(block_number, _)| *block_number);
        let block_ids = self
            .block_ids
            .iter()
            .map(|(_, block_id)| *block_id)
            .collect();
        self.metadata = Some(self.copier.complete_block(block_ids).await?);
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
                    "block copy result content length does not match source",
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
        if self.scheduled_block_ids.is_empty() {
            self.completed = true;
            self.metadata = None;
            return Ok(());
        }

        self.copier
            .abort_block(self.scheduled_block_ids.clone())
            .await?;
        self.completed = true;
        self.metadata = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use tokio::time::Duration;
    use tokio::time::sleep;

    use super::*;
    use crate::raw::oio::Copy;

    #[derive(Default)]
    struct TestState {
        ranges: HashMap<Uuid, BytesRange>,
        completed_ranges: Vec<BytesRange>,
    }

    struct TestCopy {
        state: Arc<Mutex<TestState>>,
    }

    impl BlockCopy for TestCopy {
        async fn source_metadata(&self) -> Result<Metadata> {
            Ok({
                let mut metadata = MetadataBuilder::unknown();
                metadata.set_file(4);
                metadata.build()
            })
        }

        async fn copy_once(&self) -> Result<Metadata> {
            Ok(MetadataBuilder::unknown().build())
        }

        async fn copy_block(&self, block_id: Uuid, range: BytesRange) -> Result<()> {
            if range.offset() == 0 {
                sleep(Duration::from_millis(50)).await;
            }

            self.state
                .lock()
                .expect("test state mutex poisoned")
                .ranges
                .insert(block_id, range);

            Ok(())
        }

        async fn complete_block(&self, block_ids: Vec<Uuid>) -> Result<Metadata> {
            let mut state = self.state.lock().expect("test state mutex poisoned");
            state.completed_ranges = block_ids
                .into_iter()
                .map(|block_id| state.ranges[&block_id])
                .collect();
            Ok(MetadataBuilder::unknown().build())
        }

        async fn abort_block(&self, _: Vec<Uuid>) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_block_copier_completes_blocks_in_source_order() -> Result<()> {
        let state = Arc::new(Mutex::new(TestState::default()));
        let inner = TestCopy {
            state: state.clone(),
        };
        let mut copier = BlockCopier::new(Executor::default(), inner, None, 0, 2, 2);

        while copier.next().await?.is_some() {}

        let completed_ranges = state
            .lock()
            .expect("test state mutex poisoned")
            .completed_ranges
            .clone();
        assert_eq!(
            completed_ranges,
            vec![BytesRange::new(0, Some(2)), BytesRange::new(2, Some(2))]
        );
        let metadata = copier.close().await?;
        assert!(metadata.is_file());
        assert_eq!(metadata.content_length(), 4);

        Ok(())
    }
}
