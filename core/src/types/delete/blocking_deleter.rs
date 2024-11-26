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

use crate::raw::oio::Delete;
use crate::raw::*;
use crate::*;

/// BlockingDeleter is designed to continuously remove content from storage.
///
/// It leverages batch deletion capabilities provided by storage services for efficient removal.
pub struct BlockingDeleter {
    deleter: oio::BlockingDeleter,

    max_size: usize,
    cur_size: usize,
}

impl BlockingDeleter {
    pub(crate) fn create(acc: Accessor) -> Result<Self> {
        let max_size = acc.info().full_capability().delete_max_size.unwrap_or(1);
        let (_, deleter) = acc.blocking_delete()?;

        Ok(Self {
            deleter,
            max_size,
            cur_size: 0,
        })
    }

    /// Delete a path.
    pub fn delete(&mut self, input: impl IntoDeleteInput) -> Result<()> {
        if self.cur_size >= self.max_size {
            let deleted = self.deleter.flush()?;
            self.cur_size -= deleted;
        }

        let input = input.into_delete_input();
        let mut op = OpDelete::default();
        if let Some(version) = &input.version {
            op = op.with_version(version);
        }

        self.deleter.delete(&input.path, op)?;
        self.cur_size += 1;
        Ok(())
    }

    /// Delete a stream of paths.
    pub fn delete_from<S, E>(&mut self, mut iter: S) -> Result<()>
    where
        S: Iterator<Item = Result<E>>,
        E: IntoDeleteInput,
    {
        loop {
            match iter.next() {
                Some(Ok(entry)) => {
                    self.delete(entry)?;
                }
                Some(Err(err)) => return Err(err),
                None => break,
            }
        }

        Ok(())
    }

    /// Flush the deleter, returns the number of deleted paths.
    pub fn flush(&mut self) -> Result<usize> {
        let deleted = self.deleter.flush()?;
        self.cur_size -= deleted;
        Ok(deleted)
    }

    /// Close the deleter, this will flush the deleter and wait until all paths are deleted.
    pub fn close(&mut self) -> Result<()> {
        loop {
            self.deleter.flush()?;
            if self.cur_size == 0 {
                break;
            }
        }
        Ok(())
    }
}