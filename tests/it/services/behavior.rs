// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! `behavior` intents to provide behavior tests for all storage services.
//!
//! # Note
//!
//! `behavior` requires most of the logic is correct, especially `write` and `delete`. We will not depends service specific functions to prepare the fixtures.
//!
//! For examples, we depends `write` to create a file before testing `read`. If `write` doesn't works well, we can't test `read` correctly too.

use anyhow::Result;
use futures::io::Cursor;
use futures::AsyncReadExt;
use opendal::Operator;
use rand::prelude::*;
use sha2::{Digest, Sha256};

/// TODO: Implement test files cleanup.
pub struct BehaviorTest {
    op: Operator,

    rng: ThreadRng,
}

impl BehaviorTest {
    pub fn new(op: Operator) -> Self {
        Self {
            op,
            rng: thread_rng(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.test_normal().await?;

        Ok(())
    }

    /// This case is use to test service's normal behavior.
    async fn test_normal(&mut self) -> Result<()> {
        // Step 1: Generate a random file with random size (under 4 MB).
        let path = uuid::Uuid::new_v4().to_string();
        println!("Generate a random file: {}", &path);
        let (content, size) = self.gen_bytes();

        // Step 2: Write this file
        let n = self
            .op
            .write(&path, size as u64)
            .run(Box::new(Cursor::new(content.clone())))
            .await?;
        assert_eq!(n, size, "write file");

        // Step 3: Stat this file
        let o = self.op.stat(&path).run().await?;
        assert_eq!(o.size, size as u64, "stat file");

        // Step 4: Read this file's content
        // Step 4.1: Read the whole file.
        let mut buf = Vec::new();
        let mut r = self.op.read(&path).run().await?;
        let n = r.read_to_end(&mut buf).await?;
        assert_eq!(n, size as usize, "check size in read whole file");
        assert_eq!(
            format!("{:x}", Sha256::digest(&buf)),
            format!("{:x}", Sha256::digest(&content)),
            "check hash in read whole file"
        );

        // Step 4.2: Read the file with random offset and length.
        let (offset, length) = self.gen_offset_length(size as usize);
        let mut buf = Vec::new();
        let mut r = self
            .op
            .read(&path)
            .offset(offset)
            .size(length)
            .run()
            .await?;
        r.read_to_end(&mut buf).await?;
        assert_eq!(
            format!("{:x}", Sha256::digest(&buf)),
            format!(
                "{:x}",
                Sha256::digest(&content[offset as usize..(offset + length) as usize])
            ),
            "read part file"
        );

        // Step 5: Delete this file
        let result = self.op.delete(&path).run().await;
        assert!(result.is_ok(), "delete file");

        // Step 6: Stat this file again to check if it's deleted
        let result = self.op.stat(&path).run().await;
        assert!(result.is_err(), "stat file again");
        assert!(
            matches!(
                result.err().unwrap(),
                opendal::error::Error::ObjectNotExist(_)
            ),
            "stat file again"
        );
        Ok(())
    }

    fn gen_bytes(&mut self) -> (Vec<u8>, usize) {
        let size = self.rng.gen_range(1..4 * 1024 * 1024);
        let mut content = vec![0; size as usize];
        self.rng.fill_bytes(&mut content);

        (content, size)
    }

    fn gen_offset_length(&mut self, size: usize) -> (u64, u64) {
        // Make sure at least one byte is read.
        let offset = self.rng.gen_range(0..size - 1);
        let length = self.rng.gen_range(1..(size - offset));

        (offset as u64, length as u64)
    }
}
