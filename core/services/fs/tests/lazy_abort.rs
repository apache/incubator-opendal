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

use opendal_core::{ErrorKind, Operator};
use opendal_service_fs::Fs;

#[tokio::test]
async fn abort_unused_writer_preserves_existing_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("existing");
    std::fs::write(&path, b"original contents").unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    let mut writer = op.writer("existing").await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"original contents");
    let result = writer.abort().await;
    assert_eq!(std::fs::read(&path).unwrap(), b"original contents");
    result.unwrap();
}

#[tokio::test]
async fn abort_buffered_writer_preserves_existing_file() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("existing");
    std::fs::write(&path, b"original contents").unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    let mut writer = op.writer_with("existing").chunk(8).await.unwrap();
    writer.write("new").await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"original contents");
    let result = writer.abort().await;
    assert_eq!(std::fs::read(&path).unwrap(), b"original contents");
    result.unwrap();
}

#[tokio::test]
async fn abort_unused_writer_does_not_create_file_or_parents() {
    let root = tempfile::tempdir().unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    let mut writer = op.writer("nested/new").await.unwrap();
    assert!(!root.path().join("nested").exists());
    let result = writer.abort().await;
    assert!(!root.path().join("nested").exists());
    result.unwrap();
}

#[tokio::test]
async fn abort_unused_conditional_writer_does_not_attempt_a_write() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("existing");
    std::fs::write(&path, b"original contents").unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    let mut writer = op
        .writer_with("existing")
        .if_not_exists(true)
        .await
        .unwrap();
    writer.abort().await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"original contents");
}

#[tokio::test]
async fn close_unused_writer_still_creates_empty_file() {
    let root = tempfile::tempdir().unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    let mut writer = op.writer("empty").await.unwrap();
    writer.close().await.unwrap();
    assert_eq!(
        std::fs::metadata(root.path().join("empty")).unwrap().len(),
        0
    );
}

#[tokio::test]
async fn abort_unused_atomic_writer_does_not_create_target_parents() {
    let root = tempfile::tempdir().unwrap();
    let staging = tempfile::tempdir().unwrap();
    let op = Operator::new(
        Fs::default()
            .root(root.path().to_str().unwrap())
            .atomic_write_dir(staging.path().to_str().unwrap()),
    )
    .unwrap();
    let mut writer = op.writer("nested/new").await.unwrap();
    assert!(!root.path().join("nested").exists());
    writer.abort().await.unwrap();
    assert!(!root.path().join("nested").exists());
    assert_eq!(std::fs::read_dir(staging.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn abort_started_non_atomic_writer_still_reports_unsupported() {
    let root = tempfile::tempdir().unwrap();
    let op = Operator::new(Fs::default().root(root.path().to_str().unwrap())).unwrap();
    let mut writer = op.writer("started").await.unwrap();
    writer.write("written contents").await.unwrap();
    assert!(root.path().join("started").exists());
    assert_eq!(
        writer.abort().await.unwrap_err().kind(),
        ErrorKind::Unsupported
    );
}
