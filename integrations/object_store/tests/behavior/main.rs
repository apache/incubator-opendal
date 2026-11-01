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

mod delete;
mod get;
mod put;
mod utils;

use libtest_mimic::Arguments;

fn main() -> anyhow::Result<()> {
    let Ok(Some(op)) = opendal::tests::init_test_service() else {
        return Ok(());
    };
    let store = object_store_opendal::OpendalStore::new(op);

    let mut tests = Vec::new();
    delete::tests(&store, &mut tests);
    put::tests(&store, &mut tests);
    get::tests(&store, &mut tests);

    let args = Arguments::from_args();
    let conclusion = libtest_mimic::run(&args, tests);
    conclusion.exit();
}
