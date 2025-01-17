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

use std::fs;

use common::OfsTestContext;
use test_context::test_context;
use walkdir::WalkDir;

#[test_context(OfsTestContext)]
#[test]
fn test_path(ctx: &mut OfsTestContext) {
    let actual_entries = [
        ("dir1", false),
        ("dir2", false),
        ("dir3", false),
        ("dir3/dir4", false),
        ("dir3/dir5", false),
        ("dir3/file3", true),
        ("dir3/file4", true),
        ("file1", true),
        ("file2", true),
    ]
    .into_iter()
    .map(|(x, y)| (x.to_string(), y))
    .collect::<Vec<_>>();

    for (path, is_file) in actual_entries.iter() {
        let path = ctx.mount_point.path().join(path);
        match is_file {
            true => fs::write(path, "hello").unwrap(),
            false => fs::create_dir(path).unwrap(),
        }
    }

    assert_eq!(
        actual_entries,
        WalkDir::new(ctx.mount_point.path())
            .min_depth(1)
            .max_depth(2)
            .sort_by_file_name()
            .into_iter()
            .map(|x| {
                let x = x.unwrap();
                (
                    x.path()
                        .strip_prefix(&ctx.mount_point)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                    x.file_type().is_file(),
                )
            })
            .collect::<Vec<_>>()
    );
}
