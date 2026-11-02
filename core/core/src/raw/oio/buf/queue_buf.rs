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
use std::mem;

use bytes::Buf;

use crate::*;

/// QueueBuf is a queue of [`Buffer`].
///
/// QueueBuf allows:
///
/// - Storing multiple buffers without copying bytes.
/// - Consuming buffers in order.
///
/// QueueBuf mainly provides the following operations:
///
/// - `push`: Push a new buffer in the queue.
/// - `collect`: Collect all buffer in the queue as a new [`Buffer`]
/// - `advance`: Advance the queue by `cnt` bytes.
#[derive(Clone, Default)]
pub struct QueueBuf(VecDeque<Buffer>);

impl QueueBuf {
    /// Create a new buffer queue.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push new [`Buffer`] into the queue.
    #[inline]
    pub fn push(&mut self, buf: Buffer) {
        if buf.is_empty() {
            return;
        }

        self.0.push_back(buf);
    }

    /// Total bytes size inside the buffer queue.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.iter().map(|b| b.len()).sum()
    }

    /// Is the buffer queue empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Take the entire buffer queue and leave `self` in empty states.
    #[inline]
    pub fn take(&mut self) -> QueueBuf {
        mem::take(self)
    }

    /// Build a new [`Buffer`] from the queue.
    ///
    /// If the queue is empty, it will return an empty buffer. Otherwise, it will iterate over all
    /// buffers and collect them into a new buffer.
    ///
    /// # Notes
    ///
    /// There are allocation overheads when collecting multiple buffers into a new buffer. But
    /// most of them should be acceptable since we can expect the item length of buffers are slower
    /// than 4k.
    #[inline]
    pub fn collect(mut self) -> Buffer {
        if self.0.is_empty() {
            Buffer::new()
        } else if self.0.len() == 1 {
            self.0.pop_front().unwrap()
        } else {
            self.0.into_iter().flatten().collect()
        }
    }

    /// Advance the buffer queue by `cnt` bytes.
    #[inline]
    pub fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.len(), "cannot advance past {cnt} bytes");

        let mut new_cnt = cnt;
        while new_cnt > 0 {
            let buf = self.0.front_mut().expect("buffer must be valid");
            if new_cnt < buf.remaining() {
                buf.advance(new_cnt);
                break;
            } else {
                new_cnt -= buf.remaining();
                self.0.pop_front();
            }
        }
    }

    /// Clear the buffer queue.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }
}
