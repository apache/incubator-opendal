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

use crate::utils::build_trail;
use bytes::Bytes;
use libtest_mimic::Trial;
use object_store::ObjectStoreExt;
use object_store::path::Path;
use object_store_opendal::OpendalStore;

pub fn tests(store: &OpendalStore, tests: &mut Vec<Trial>) {
    tests.push(build_trail("test_delete", store, test_delete));
}

pub async fn test_delete(store: OpendalStore) -> anyhow::Result<()> {
    let location = Path::from("data/test.txt");
    let value = Bytes::from("Hello, world!");
    store.put(&location, value.clone().into()).await?;

    store.delete(&location).await?;

    let err = store.get(&location).await.err().unwrap();
    assert!(matches!(err, object_store::Error::NotFound { .. }));

    Ok(())
}
