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

use futures::TryStreamExt;
use opendal::Operator;
use opendal::services::Git;
use sha2::{Digest, Sha256};

const REPOSITORY: &str = "https://huggingface.co/hf-internal-testing/tiny-random-bert";
const COMMIT: &str = "f171d7baecaf37b5da5a3616d8833b9969753535";
const SHA256: &str = "9922e8996d0c7e24c7f4e7a5d9c5b7303549f4ee94de0f1138b103014b51be13";
const PATH: &str = "pytorch_model.bin";

#[tokio::test]
async fn public_git_lfs_stream_range_and_pointer() {
    let operator = Operator::new(Git::default().repository(REPOSITORY).reference(COMMIT))
        .unwrap_or_else(|error| panic!("operator build failed: {:?}", error.kind()));
    let metadata = operator
        .stat(PATH)
        .await
        .unwrap_or_else(|error| panic!("stat failed: {:?}", error.kind()));
    assert_eq!(metadata.content_length(), 540_217);
    assert!(
        operator
            .stat("/")
            .await
            .unwrap_or_else(|error| panic!("root stat failed: {:?}", error.kind()))
            .mode()
            .is_dir()
    );
    let entries = operator
        .list("/")
        .await
        .unwrap_or_else(|error| panic!("list failed: {:?}", error.kind()));
    assert!(
        entries
            .iter()
            .any(|entry| entry.path() == PATH && entry.metadata().content_length() == 540_217)
    );

    let reader = operator
        .reader(PATH)
        .await
        .unwrap_or_else(|error| panic!("reader failed: {:?}", error.kind()));
    let mut stream = reader
        .into_stream(..)
        .await
        .unwrap_or_else(|error| panic!("stream open failed: {:?}", error.kind()));
    let mut payload = Vec::new();
    let mut chunks = 0;
    while let Some(buffer) = stream
        .try_next()
        .await
        .unwrap_or_else(|error| panic!("stream read failed: {:?}", error.kind()))
    {
        chunks += 1;
        payload.extend_from_slice(&buffer.to_bytes());
    }
    assert_eq!(payload.len(), 540_217);
    assert_eq!(format!("{:x}", Sha256::digest(&payload)), SHA256);

    let range = operator
        .read_with(PATH)
        .range(101..509)
        .await
        .unwrap_or_else(|error| panic!("range read failed: {:?}", error.kind()));
    assert_eq!(range.to_bytes().as_ref(), &payload[101..509]);

    let raw = Operator::new(
        Git::default()
            .repository(REPOSITORY)
            .reference(COMMIT)
            .resolve_lfs(false),
    )
    .unwrap_or_else(|error| panic!("raw operator build failed: {:?}", error.kind()));
    let pointer = raw
        .read(PATH)
        .await
        .unwrap_or_else(|error| panic!("raw pointer read failed: {:?}", error.kind()));
    let expected =
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{SHA256}\nsize 540217\n");
    assert_eq!(pointer.to_bytes().as_ref(), expected.as_bytes());
    println!(
        "GIT_LFS_LIVE full_bytes={} chunks={chunks} sha256={SHA256} range_bytes={} raw_pointer_bytes={} checks=3",
        payload.len(),
        range.len(),
        pointer.len()
    );
}
