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

use std::sync::LazyLock;

use libfuzzer_sys::arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use opendal::Operator;
use opendal::tests::TEST_RUNTIME;
use opendal::tests::init_test_service;

#[derive(Debug, Clone, Arbitrary)]
struct FuzzInput {
    path: String,
    target: String,
}

static OPERATOR: LazyLock<Operator> = LazyLock::new(|| {
    if let Some(op) = init_test_service().expect("operator init must succeed") {
        return op;
    }

    log::warn!("OPENDAL_TEST is not set; falling back to a temporary fs operator");
    let root = std::env::temp_dir().join(format!("opendal-fuzz-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create fuzz root dir must succeed");
    Operator::new(opendal::services::Fs::default().root(&root.to_string_lossy()))
        .expect("operator init must succeed")
});

async fn fuzz_path(op: Operator, input: FuzzInput) {
    let _ = op.write(&input.path, "data".as_bytes()).await;
    let _ = op.stat(&input.path).await;
    let _ = op.list(&input.path).await;
    let _ = op.create_dir(&input.path).await;
    let _ = op.copy(&input.path, &input.target).await;
    let _ = op.rename(&input.path, &input.target).await;
    let _ = op.delete(&input.target).await;
    let _ = op.delete(&input.path).await;
}

fuzz_target!(|input: FuzzInput| {
    let _ = logforth::starter_log::stderr().try_apply();

    let op = OPERATOR.clone();
    TEST_RUNTIME.block_on(async {
        fuzz_path(op, input.clone()).await;
    })
});
