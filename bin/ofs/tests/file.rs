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

#![cfg(any(target_os = "linux", target_os = "freebsd", target_os = "macos"))]

mod common;

use std::fs::File;
use std::fs::OpenOptions;
use std::fs::{self};
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::thread;
use std::time::Duration;

use common::OfsTestContext;
use test_context::test_context;

static TEST_TEXT: &str = include_str!("../Cargo.toml");

#[test_context(OfsTestContext)]
#[test]
fn test_file(ctx: &mut OfsTestContext) {
    let path = ctx.mount_point.path().join("test_file.txt");
    let mut file = File::create(&path).unwrap();

    file.write_all(TEST_TEXT.as_bytes()).unwrap();
    drop(file);

    let mut file = File::open(&path).unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, TEST_TEXT);
    drop(file);

    fs::remove_file(path).unwrap();
}

#[test_context(OfsTestContext)]
#[test]
fn test_file_append(ctx: &mut OfsTestContext) {
    if !ctx.capability.write_can_append {
        // wait for ofs to be ready
        thread::sleep(Duration::from_secs(1));
        return;
    }

    let path = ctx.mount_point.path().join("test_file_append.txt");
    let mut file = File::create(&path).unwrap();

    file.write_all(TEST_TEXT.as_bytes()).unwrap();
    drop(file);

    let mut file = File::options().append(true).open(&path).unwrap();
    file.write_all(b"test").unwrap();
    drop(file);

    let mut file = File::open(&path).unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, TEST_TEXT.to_owned() + "test");
    drop(file);

    fs::remove_file(path).unwrap();
}

#[test_context(OfsTestContext)]
#[test]
fn test_file_seek(ctx: &mut OfsTestContext) {
    let path = ctx.mount_point.path().join("test_file_seek.txt");
    let mut file = File::create(&path).unwrap();

    file.write_all(TEST_TEXT.as_bytes()).unwrap();
    drop(file);

    let mut file = File::open(&path).unwrap();
    file.seek(SeekFrom::Start(TEST_TEXT.len() as u64 / 2))
        .unwrap();
    let mut buf = String::new();
    file.read_to_string(&mut buf).unwrap();
    assert_eq!(buf, TEST_TEXT[TEST_TEXT.len() / 2..]);
    drop(file);

    fs::remove_file(path).unwrap();
}

#[test_context(OfsTestContext)]
#[test]
fn test_file_truncate(ctx: &mut OfsTestContext) {
    let path = ctx.mount_point.path().join("test_file_truncate.txt");
    let mut file = File::create(&path).unwrap();
    file.write_all(TEST_TEXT.as_bytes()).unwrap();
    drop(file);

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&path)
        .unwrap();
    file.write_all(TEST_TEXT[..TEST_TEXT.len() / 2].as_bytes())
        .unwrap();
    drop(file);

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        TEST_TEXT[..TEST_TEXT.len() / 2]
    );

    fs::remove_file(path).unwrap();
}
