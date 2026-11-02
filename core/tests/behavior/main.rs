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

// utils that used by tests
#[macro_use]
mod utils;

pub use utils::*;

mod async_compose;
mod async_copy;
mod async_create_dir;
mod async_delete;
mod async_list;
mod async_presign;
mod async_read;
mod async_rename;
mod async_restore;
mod async_stat;
mod async_write;

// External dependencies
use libtest_mimic::Arguments;
use libtest_mimic::Trial;
use logforth::append::Testing;
use logforth::filter::rustlog::RustLogFilterBuilder;
use logforth::layout::TextLayout;
use opendal::tests::TEST_RUNTIME;
use opendal::tests::init_test_service;
use opendal::*;

pub static TEST_FIXTURE: Fixture = Fixture::new();

fn main() -> anyhow::Result<()> {
    let args = Arguments::from_args();

    let op = if let Some(op) = init_test_service()? {
        op
    } else {
        return Ok(());
    };

    let mut tests = Vec::new();

    async_copy::tests(&op, &mut tests);
    async_compose::tests(&op, &mut tests);
    async_create_dir::tests(&op, &mut tests);
    async_delete::tests(&op, &mut tests);
    async_list::tests(&op, &mut tests);
    async_presign::tests(&op, &mut tests);
    async_read::tests(&op, &mut tests);
    async_rename::tests(&op, &mut tests);
    async_restore::tests(&op, &mut tests);
    async_stat::tests(&op, &mut tests);
    async_write::tests(&op, &mut tests);

    // Don't init logging while building operator which may break cargo
    // nextest output
    let _ = logforth::starter_log::builder()
        .dispatch(|d| {
            d.filter(RustLogFilterBuilder::from_default_env().build())
                .append(Testing::default().with_layout(TextLayout::default()))
        })
        .try_apply();

    let conclusion = libtest_mimic::run(&args, tests);

    // Cleanup the fixtures.
    TEST_RUNTIME.block_on(TEST_FIXTURE.cleanup(op.clone()));
    drop(op);

    conclusion.exit()
}
