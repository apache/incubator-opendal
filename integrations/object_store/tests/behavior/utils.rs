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

use libtest_mimic::{Failed, Trial};
use object_store_opendal::OpendalStore;
use opendal::raw::MaybeSend;
use opendal::tests::TEST_RUNTIME;
use std::future::Future;

pub fn build_trail<F, Fut>(name: &str, store: &OpendalStore, f: F) -> Trial
where
    F: FnOnce(OpendalStore) -> Fut + MaybeSend + 'static,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let handle = TEST_RUNTIME.handle().clone();

    let store = store.clone();
    Trial::test(format!("behavior::{name}"), move || {
        handle
            .block_on(f(store))
            .map_err(|err| Failed::from(err.to_string()))
    })
}

pub fn new_file_path(dir: &str) -> String {
    format!("{}/{}", dir, uuid::Uuid::new_v4())
}
