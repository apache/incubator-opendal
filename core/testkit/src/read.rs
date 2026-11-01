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
use opendal_core::*;
use rand::Rng;
use rand::rng;

use crate::utils::sha256_digest;

/// A reader operation executed by [`ReadChecker`].
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReadAction {
    /// Read `size` bytes beginning at `offset`.
    ///
    /// The first field is the offset and the second field is the requested
    /// size. A reader can return fewer bytes at the end of an object.
    Read(usize, usize),
}

/// Generates reference data and verifies reads against it.
pub struct ReadChecker {
    /// Data that callers write to the storage before checking reads.
    raw_data: Bytes,
}

impl ReadChecker {
    /// Create a checker containing `size` bytes of random reference data.
    ///
    /// Random input makes the checker sensitive to misplaced or repeated data;
    /// callers should not depend on the generated content.
    pub fn new(size: usize) -> Self {
        let mut rng = rng();
        let mut data = vec![0; size];
        rng.fill_bytes(&mut data);

        let raw_data = Bytes::from(data);

        Self { raw_data }
    }

    /// Return the reference data that should be written before a check.
    pub fn data(&self) -> Bytes {
        self.raw_data.clone()
    }

    /// check_read checks the correctness of the read process after a read action.
    ///
    /// - buf_size is the read action's buf size.
    /// - output is the output of this read action.
    fn check_read(&self, offset: usize, size: usize, output: &[u8]) {
        if size == 0 {
            assert_eq!(
                output.len(),
                0,
                "check read failed: output must be empty if buf_size is 0"
            );
            return;
        }

        if size > 0 && output.is_empty() {
            assert!(
                offset >= self.raw_data.len(),
                "check read failed: no data read means cur must outsides of ranged_data",
            );
            return;
        }

        assert!(
            offset + output.len() <= self.raw_data.len(),
            "check read failed: cur + output length must be less than ranged_data length, offset: {}, output: {}, ranged_data: {}",
            offset,
            output.len(),
            self.raw_data.len(),
        );

        let expected = &self.raw_data[offset..offset + output.len()];

        if output != expected {
            assert_eq!(
                sha256_digest(output),
                sha256_digest(expected),
                "check read failed: output bs is different with expected bs",
            );
        }
    }

    /// Execute `actions` and verify each result against the reference data.
    ///
    /// This method panics if a read fails or returns incorrect data.
    pub async fn check(&mut self, r: Reader, actions: &[ReadAction]) {
        for action in actions {
            match *action {
                ReadAction::Read(offset, size) => {
                    let bs = r
                        .read(offset as u64..(offset + size) as u64)
                        .await
                        .expect("read must success");
                    self.check_read(offset, size, bs.to_bytes().as_ref());
                }
            }
        }
    }
}
