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

use std::collections::HashSet;
use std::future::Future;

use crate::raw::*;
use crate::*;

/// BatchDelete is used to implement [`oio::Delete`] based on batch delete operation.
///
/// OneShotDeleter will perform delete operation while calling `close`.
pub trait BatchDelete: Send + Sync + Unpin + 'static {
    /// delete_once delete one path at once.
    ///
    /// Implementations should make sure that the data is deleted correctly at once.
    ///
    /// BatchDeleter may call this method while there are only one path to delete.
    fn delete_once(
        &self,
        path: String,
        args: OpDelete,
    ) -> impl Future<Output = Result<()>> + MaybeSend;

    /// delete_batch delete multiple paths at once.
    ///
    /// - Implementations should make sure that the length of `batch` equals to the return result's length.
    /// - Implementations should return error no path is deleted.
    fn delete_batch(
        &self,
        batch: Vec<(String, OpDelete)>,
    ) -> impl Future<Output = Result<BatchDeleteResult>> + MaybeSend;
}

/// BatchDeleteResult is the result of batch delete operation.
#[derive(Default)]
pub struct BatchDeleteResult {
    /// Collection of successful deletions, containing tuples of (path, args)
    pub succeeded: Vec<(String, OpDelete)>,
    /// Collection of failed deletions, containing tuples of (path, args, error)
    pub failed: Vec<(String, OpDelete, Error)>,
}

/// BatchDeleter is used to implement [`oio::Delete`] based on batch delete.
pub struct BatchDeleter<D: BatchDelete> {
    inner: D,
    buffer: HashSet<(String, OpDelete)>,
    max_batch_size: usize,
}

impl<D: BatchDelete> BatchDeleter<D> {
    /// Create a new batch deleter.
    pub fn new(inner: D, max_batch_size: Option<usize>) -> Self {
        debug_assert!(
            max_batch_size.is_some(),
            "BatchDeleter requires delete_max_size to be configured"
        );
        let max_batch_size = max_batch_size.unwrap_or(1);

        Self {
            inner,
            buffer: HashSet::default(),
            max_batch_size,
        }
    }

    async fn flush_buffer(&mut self) -> Result<usize> {
        if self.buffer.is_empty() {
            return Ok(0);
        }

        if self.buffer.len() == 1 {
            let (path, args) = self
                .buffer
                .iter()
                .next()
                .expect("the delete buffer size must be 1")
                .clone();
            self.inner.delete_once(path, args).await?;
            self.buffer.clear();
            return Ok(1);
        }

        let batch = self.buffer.iter().cloned().collect();
        let result = self.inner.delete_batch(batch).await?;

        if result.succeeded.is_empty() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "batch delete returned zero successes",
            ));
        }
        if result.succeeded.len() + result.failed.len() != self.buffer.len() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "batch delete result size mismatch",
            ));
        }

        let mut deleted = 0;
        for i in result.succeeded {
            self.buffer.remove(&i);
            deleted += 1;
        }

        for (path, op, err) in result.failed {
            if !err.is_temporary() {
                return Err(err
                    .with_context("path", path)
                    .with_context("version", op.version().unwrap_or("<latest>")));
            }
        }

        Ok(deleted)
    }
}

impl<D: BatchDelete> oio::Delete for BatchDeleter<D> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.buffer.insert((path.to_string(), args));
        if self.buffer.len() >= self.max_batch_size {
            let _ = self.flush_buffer().await?;
            return Ok(());
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        loop {
            let deleted = self.flush_buffer().await?;

            if self.buffer.is_empty() {
                break;
            }

            if deleted == 0 {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "batch delete made no progress while buffer remains",
                ));
            }
        }

        Ok(())
    }
}
