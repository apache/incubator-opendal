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

#![no_main]

use std::io::SeekFrom;

use bytes::Bytes;
use libfuzzer_sys::arbitrary::Arbitrary;
use libfuzzer_sys::arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use opendal::raw::oio::ReadExt;
use opendal::raw::BytesRange;
use opendal::Operator;
use opendal::Result;
use rand::prelude::*;
use sha2::Digest;
use sha2::Sha256;

mod utils;

const MAX_DATA_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
enum ReadAction {
    Read { size: usize },
    Seek(SeekFrom),
    Next,
}

#[derive(Debug, Clone)]
struct FuzzInput {
    size: usize,
    range: BytesRange,
    actions: Vec<ReadAction>,
}

impl Arbitrary<'_> for FuzzInput {
    fn arbitrary(u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        let total_size = u.int_in_range(1..=MAX_DATA_SIZE)?;

        // TODO: it's valid that size is larger than total_size.
        let (offset, size) = match u.int_in_range(0..=3)? {
            // Full range
            0 => (None, None),
            1 => {
                let offset = u.int_in_range(0..=total_size as u64 - 1)?;
                (Some(offset), None)
            }
            2 => {
                let size = u.int_in_range(1..=total_size as u64)?;
                (None, Some(size))
            }
            3 => {
                let offset = u.int_in_range(0..=total_size as u64 - 1)?;
                let size = u.int_in_range(1..=total_size as u64 - offset)?;
                (Some(offset), Some(size))
            }
            _ => unreachable!("invalid int generated by arbitrary"),
        };
        let range = BytesRange::new(offset, size);

        let count = u.int_in_range(1..=1024)?;
        let mut actions = vec![];

        for _ in 0..count {
            let action = match u.int_in_range(0..=4)? {
                // Read
                0 => {
                    let size = u.int_in_range(0..=total_size * 2)?;
                    ReadAction::Read { size }
                }
                // Next
                1 => ReadAction::Next,
                // Seek Start
                2 => {
                    // NOTE: seek out of the end of file is valid.
                    let offset = u.int_in_range(0..=total_size * 2)?;
                    ReadAction::Seek(SeekFrom::Start(offset as u64))
                }
                // Seek Current
                3 => {
                    let offset = u.int_in_range(-(total_size as i64)..=(total_size as i64))?;
                    ReadAction::Seek(SeekFrom::Current(offset))
                }
                // Seek End
                4 => {
                    let offset = u.int_in_range(-(total_size as i64)..=(total_size as i64))?;
                    ReadAction::Seek(SeekFrom::End(offset))
                }
                _ => unreachable!("invalid int generated by arbitrary"),
            };

            actions.push(action);
        }

        Ok(FuzzInput {
            size: total_size,
            range,
            actions,
        })
    }
}

struct ReadChecker {
    /// Raw Data is the data we write to the storage.
    raw_data: Bytes,
    /// Ranged Data is the data that we read from the storage.
    ranged_data: Bytes,

    cur: usize,
}

impl ReadChecker {
    fn new(size: usize, range: BytesRange) -> Self {
        let mut rng = thread_rng();
        let mut data = vec![0; size];
        rng.fill_bytes(&mut data);

        let raw_data = Bytes::from(data);
        let ranged_data = range.apply_on_bytes(raw_data.clone());

        Self {
            raw_data,
            ranged_data,

            cur: 0,
        }
    }

    fn check_read(&mut self, n: usize, output: &[u8]) {
        if n == 0 {
            assert_eq!(
                output.len(),
                0,
                "check read failed: output bs is not empty when read size is 0"
            );
            return;
        }

        let expected = &self.ranged_data[self.cur..self.cur + n];

        // Check the read result
        assert_eq!(
            format!("{:x}", Sha256::digest(output)),
            format!("{:x}", Sha256::digest(expected)),
            "check read failed: output bs is different with expected bs",
        );

        // Update the current position
        self.cur += n;
    }

    fn check_seek(&mut self, seek_from: SeekFrom, output: Result<u64>) {
        let expected = match seek_from {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.ranged_data.len() as i64 + offset,
            SeekFrom::Current(offset) => self.cur as i64 + offset,
        };

        if expected < 0 {
            assert!(output.is_err(), "check seek failed: seek should fail");
            assert_eq!(
                output.unwrap_err().kind(),
                opendal::ErrorKind::InvalidInput,
                "check seek failed: seek result is different with expected result"
            );

            return;
        }

        assert_eq!(
            output.unwrap(),
            expected as u64,
            "check seek failed: seek result is different with expected result",
        );

        // only update the current position when seek succeed
        self.cur = expected as usize;
    }

    fn check_next(&mut self, output: Option<Bytes>) {
        if let Some(output) = output {
            assert!(
                self.cur + output.len() <= self.ranged_data.len(),
                "check next failed: output bs is larger than remaining bs",
            );

            assert_eq!(
                format!("{:x}", Sha256::digest(&output)),
                format!(
                    "{:x}",
                    Sha256::digest(&self.ranged_data[self.cur..self.cur + output.len()])
                ),
                "check next failed: output bs is different with expected bs",
            );

            // update the current position
            self.cur += output.len();
        } else {
            assert!(
                self.cur >= self.ranged_data.len(),
                "check next failed: output bs is None, we still have bytes to read",
            )
        }
    }
}

async fn fuzz_reader(op: Operator, input: FuzzInput) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let mut checker = ReadChecker::new(input.size, input.range);
    op.write(&path, checker.raw_data.clone()).await?;

    let mut o = op.reader_with(&path).range(input.range.to_range()).await?;

    for action in input.actions {
        match action {
            ReadAction::Read { size } => {
                let mut buf = vec![0; size];
                let n = o.read(&mut buf).await?;
                checker.check_read(n, &buf[..n]);
            }

            ReadAction::Seek(seek_from) => {
                let res = o.seek(seek_from).await;
                checker.check_seek(seek_from, res);
            }

            ReadAction::Next => {
                let res = o.next().await.transpose()?;
                checker.check_next(res);
            }
        }
    }

    op.delete(&path).await?;
    Ok(())
}

fuzz_target!(|input: FuzzInput| {
    let _ = dotenvy::dotenv();

    let runtime = tokio::runtime::Runtime::new().expect("init runtime must succeed");

    for op in utils::init_services() {
        runtime.block_on(async {
            fuzz_reader(op, input.clone())
                .await
                .unwrap_or_else(|_| panic!("fuzz reader must succeed"));
        })
    }
});
