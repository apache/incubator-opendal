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

use bytes::Bytes;
use bytes::BytesMut;
use rand::Rng;
use rand::rng;

use crate::utils::sha256_digest;

/// A writer operation used by write behavior tests.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum WriteAction {
    /// Write a buffer with the given size.
    ///
    /// A writer can accept the buffer incrementally even though the action
    /// supplies it as one logical chunk.
    Write(usize),
}

/// Generates write chunks and verifies the combined stored data.
pub struct WriteChecker {
    chunks: Vec<Bytes>,
    data: Bytes,
}

impl WriteChecker {
    /// Create random chunks with the requested `sizes`.
    pub fn new(sizes: Vec<usize>) -> Self {
        let mut rng = rng();

        let mut chunks = Vec::with_capacity(sizes.len());

        for size in sizes {
            let mut bs = vec![0u8; size];
            rng.fill_bytes(&mut bs);
            chunks.push(Bytes::from(bs));
        }

        let data = chunks.iter().fold(BytesMut::new(), |mut acc, x| {
            acc.extend_from_slice(x);
            acc
        });

        WriteChecker {
            chunks,
            data: data.freeze(),
        }
    }

    /// Return the chunks to write, in order.
    pub fn chunks(&self) -> &[Bytes] {
        &self.chunks
    }

    /// Verify that `actual` equals the concatenated generated chunks.
    pub fn check(&self, actual: &[u8]) {
        if actual != self.data.as_ref() {
            assert_eq!(
                sha256_digest(actual),
                sha256_digest(&self.data),
                "check failed: result is not expected"
            );
        }
    }
}
