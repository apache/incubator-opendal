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
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::{self};
use std::sync::Mutex;

use bytes::BytesMut;

/// PooledBuf is a buffer pool that designed for reusing already allocated bufs.
///
/// It works as best-effort that tries to reuse the buffer if possible. It
/// won't block the thread if the pool is locked, just returning a new buffer
/// or dropping existing buffer.
pub struct PooledBuf {
    pool: Mutex<VecDeque<BytesMut>>,
    size: usize,
    initial_capacity: usize,
}

impl Debug for PooledBuf {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PooledBuf")
            .field("size", &self.size)
            .field("initial_capacity", &self.initial_capacity)
            .finish_non_exhaustive()
    }
}

impl PooledBuf {
    /// Create a new buffer pool with a given size.
    pub fn new(size: usize) -> Self {
        Self {
            pool: Mutex::new(VecDeque::with_capacity(size)),
            size,
            initial_capacity: 0,
        }
    }

    /// Set the initial capacity of the buffer.
    ///
    /// The default value is 0.
    pub fn with_initial_capacity(mut self, initial_capacity: usize) -> Self {
        self.initial_capacity = initial_capacity;
        self
    }

    /// Get a [`BytesMut`] from the pool.
    ///
    /// It's guaranteed that the buffer is empty.
    pub fn get(&self) -> BytesMut {
        // We don't want to block the thread if the pool is locked.
        //
        // Just returning a new buffer in this case.
        let Ok(mut pool) = self.pool.try_lock() else {
            return BytesMut::with_capacity(self.initial_capacity);
        };

        if let Some(buf) = pool.pop_front() {
            buf
        } else {
            BytesMut::with_capacity(self.initial_capacity)
        }
    }

    /// Put a [`BytesMut`] back to the pool.
    pub fn put(&self, mut buf: BytesMut) {
        // We don't want to block the thread if the pool is locked.
        //
        // Just dropping the buffer in this case.
        let Ok(mut pool) = self.pool.try_lock() else {
            return;
        };

        if pool.len() < self.size {
            // Clean the buffer before putting it back to the pool.
            buf.clear();
            pool.push_back(buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::BufMut;

    use super::*;

    #[test]
    fn test_pooled_buf() {
        let pool = PooledBuf::new(2);

        let mut buf1 = pool.get();
        buf1.put_slice(b"hello, world!");

        let mut buf2 = pool.get();
        buf2.reserve(1024);

        pool.put(buf1);
        pool.put(buf2);

        let buf3 = pool.get();
        assert_eq!(buf3.len(), 0);
        assert_eq!(buf3.capacity(), 13);

        let buf4 = pool.get();
        assert_eq!(buf4.len(), 0);
        assert_eq!(buf4.capacity(), 1024);
    }
}
