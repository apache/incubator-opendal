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

use std::mem;
use std::sync::Mutex;

use futures::Future;
use libtest_mimic::Failed;
use libtest_mimic::Trial;
use opendal::raw::*;
use opendal::tests::TEST_RUNTIME;
use opendal::*;
use rand::distr::uniform::SampleRange;
use rand::prelude::*;
use rand::rng;
use sha2::Digest;
use sha2::Sha256;

const DEFAULT_CONTENT_MAX_SIZE: usize = 4 * 1024 * 1024;

pub fn sha256_digest(data: impl AsRef<[u8]>) -> String {
    use std::fmt::Write;

    let digest = Sha256::digest(data);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to String must succeed");
    }
    output
}

pub fn gen_bytes_with_range(range: impl SampleRange<usize>) -> (Vec<u8>, usize) {
    let mut rng = rng();

    let size = rng.random_range(range);
    let mut content = vec![0; size];
    rng.fill_bytes(&mut content);

    (content, size)
}

pub fn gen_bytes(cap: Capability) -> (Vec<u8>, usize) {
    gen_bytes_with_range(1..content_max_size(cap))
}

pub fn gen_fixed_bytes(size: usize) -> Vec<u8> {
    let (content, _) = gen_bytes_with_range(size..=size);

    content
}

pub fn gen_offset_length(size: usize) -> (u64, u64) {
    let mut rng = rng();

    // Make sure at least one byte is read.
    let offset = rng.random_range(0..size - 1);
    let length = rng.random_range(1..(size - offset));

    (offset as u64, length as u64)
}

/// Build a new async trail as a test case.
pub fn build_async_trial<F, Fut>(name: &str, op: &Operator, f: F) -> Trial
where
    F: FnOnce(Operator) -> Fut + MaybeSend + 'static,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let handle = TEST_RUNTIME.handle().clone();
    let op = op.clone();

    Trial::test(format!("behavior::{name}"), move || {
        handle
            .block_on(f(op))
            .map_err(|err| Failed::from(err.to_string()))
    })
}

#[macro_export]
macro_rules! async_trials {
    ($op:ident, $($test:ident),*) => {
        vec![$(
            build_async_trial(stringify!($test), $op, $test),
        )*]
    };
}

pub struct Fixture {
    pub paths: Mutex<Vec<String>>,
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

impl Fixture {
    /// Create a new fixture
    pub const fn new() -> Self {
        Self {
            paths: Mutex::new(vec![]),
        }
    }

    /// Add a path.
    pub fn add_path(&self, path: String) {
        self.paths.lock().unwrap().push(path);
    }

    /// Create a new dir path
    pub fn new_dir_path(&self) -> String {
        let path = format!("{}/", uuid::Uuid::new_v4());
        self.paths.lock().unwrap().push(path.clone());

        path
    }

    /// Create a new file path
    pub fn new_file_path(&self) -> String {
        let path = format!("{}", uuid::Uuid::new_v4());
        self.paths.lock().unwrap().push(path.clone());

        path
    }

    /// Create a new file with random content
    pub fn new_file(&self, op: impl Into<Operator>) -> (String, Vec<u8>, usize) {
        let max_size = content_max_size(op.into().info().capability());

        self.new_file_with_range(uuid::Uuid::new_v4().to_string(), 1..max_size)
    }

    pub fn new_file_with_path(
        &self,
        op: impl Into<Operator>,
        path: &str,
    ) -> (String, Vec<u8>, usize) {
        let max_size = content_max_size(op.into().info().capability());

        self.new_file_with_range(path, 1..max_size)
    }

    /// Create a new file with random content in range.
    fn new_file_with_range(
        &self,
        path: impl Into<String>,
        range: impl SampleRange<usize>,
    ) -> (String, Vec<u8>, usize) {
        let path = path.into();
        self.paths.lock().unwrap().push(path.clone());

        let mut rng = rng();

        let size = rng.random_range(range);
        let mut content = vec![0; size];
        rng.fill_bytes(&mut content);

        (path, content, size)
    }

    /// Perform cleanup
    pub async fn cleanup(&self, op: impl Into<Operator>) {
        let op = op.into();
        // Don't cleanup data if delete is not supported
        if !op.info().capability().delete {
            return;
        }

        let paths: Vec<_> = mem::take(self.paths.lock().unwrap().as_mut());
        // Don't call delete if paths is empty
        if paths.is_empty() {
            return;
        }

        // We try our best to clean up fixtures, but won't panic if failed.
        let _ = op.delete_iter(paths).await;
    }
}

fn content_max_size(cap: Capability) -> usize {
    cap.write_total_max_size
        .map_or(DEFAULT_CONTENT_MAX_SIZE, |size| {
            size.min(DEFAULT_CONTENT_MAX_SIZE)
        })
}
