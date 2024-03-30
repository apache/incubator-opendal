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

mod common;

use std::io::SeekFrom;

use common::OfsTestContext;

use test_context::test_context;
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

static TEST_TEXT: &str = include_str!("../Cargo.toml");

#[test_context(OfsTestContext)]
#[tokio::test]
async fn test_file(ctx: &mut OfsTestContext) {
    let path = ctx.mount_point.path().join("test_file.txt");
    let mut file = File::create(&path).await.unwrap();

    file.write_all(TEST_TEXT.as_bytes()).await.unwrap();
    drop(file);

    let mut file = File::open(&path).await.unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).await.unwrap();
    assert_eq!(buf, TEST_TEXT);
    drop(file);

    fs::remove_file(path).await.unwrap();
}

#[test_context(OfsTestContext)]
#[tokio::test]
async fn test_file_append(ctx: &mut OfsTestContext) {
    let path = ctx.mount_point.path().join("test_file_append.txt");
    let mut file = File::create(&path).await.unwrap();

    file.write_all(TEST_TEXT.as_bytes()).await.unwrap();
    drop(file);

    let mut file = File::options().append(true).open(&path).await.unwrap();
    file.write_all(b"test").await.unwrap();
    drop(file);

    let mut file = File::open(&path).await.unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).await.unwrap();
    assert_eq!(buf, TEST_TEXT.to_owned() + "test");
    drop(file);

    fs::remove_file(path).await.unwrap();
}

#[test_context(OfsTestContext)]
#[tokio::test]
async fn test_file_seek(ctx: &mut OfsTestContext) {
    let path = ctx.mount_point.path().join("test_file_seek.txt");
    let mut file = File::create(&path).await.unwrap();

    file.write_all(TEST_TEXT.as_bytes()).await.unwrap();
    drop(file);

    let mut file = File::open(&path).await.unwrap();
    file.seek(SeekFrom::Start(TEST_TEXT.len() as u64 / 2))
        .await
        .unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).await.unwrap();
    assert_eq!(buf, TEST_TEXT[TEST_TEXT.len() / 2..]);
    drop(file);

    fs::remove_file(path).await.unwrap();
}
