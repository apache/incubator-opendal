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

use crate::utils::{build_trail, new_file_path};
use anyhow::Result;
use bytes::Bytes;
use libtest_mimic::Trial;
use object_store::{ObjectStore, ObjectStoreExt, PutMode, UpdateVersion};
use object_store_opendal::OpendalStore;

pub fn tests(store: &OpendalStore, tests: &mut Vec<Trial>) {
    tests.push(build_trail("test_basic_put", store, test_basic_put));
    tests.push(build_trail(
        "test_put_opts_with_overwrite",
        store,
        test_put_opts_with_overwrite,
    ));
    tests.push(build_trail(
        "test_put_opts_with_create",
        store,
        test_put_opts_with_create,
    ));
    tests.push(build_trail(
        "test_put_opts_with_update",
        store,
        test_put_opts_with_update,
    ));
}

pub async fn test_basic_put(store: OpendalStore) -> Result<()> {
    let location = new_file_path("data").into();
    let value = Bytes::from("Hello, world!");
    store.put(&location, value.clone().into()).await?;

    let data = store.get(&location).await?.bytes().await?;
    assert_eq!(value, data);

    store.delete(&location).await?;
    Ok(())
}

pub async fn test_put_opts_with_overwrite(store: OpendalStore) -> Result<()> {
    let location = new_file_path("data").into();
    let value = Bytes::from("Hello, world!");
    store.put(&location, value.clone().into()).await?;

    let new_value = Bytes::from("Hello, world! 2");
    let opts = object_store::PutOptions {
        mode: PutMode::Overwrite,
        ..Default::default()
    };
    store
        .put_opts(&location, new_value.clone().into(), opts)
        .await?;

    let data = store.get(&location).await?.bytes().await?;
    assert_eq!(new_value, data);

    store.delete(&location).await?;
    Ok(())
}

pub async fn test_put_opts_with_create(store: OpendalStore) -> Result<()> {
    if !store.info().capability().write_with_if_not_exists {
        return Ok(());
    }

    let location = new_file_path("data").into();
    let value = Bytes::from("Hello, world!");

    let opts = object_store::PutOptions {
        mode: PutMode::Create,
        ..Default::default()
    };
    let ret = store
        .put_opts(&location, value.clone().into(), opts.clone())
        .await;
    assert!(ret.is_ok());

    let ret = store.put_opts(&location, value.clone().into(), opts).await;
    assert!(ret.is_err());
    assert!(matches!(
        ret.unwrap_err(),
        object_store::Error::AlreadyExists { .. }
    ));

    store.delete(&location).await?;
    Ok(())
}

pub async fn test_put_opts_with_update(store: OpendalStore) -> Result<()> {
    if !store.info().capability().write_with_if_match {
        return Ok(());
    }

    let location = new_file_path("data").into();
    let value = Bytes::from("Hello, world!");
    store.put(&location, value.clone().into()).await?;
    let meta = store.head(&location).await?;

    let new_value = Bytes::from("Hello, world! 2");
    let put_opts = object_store::PutOptions {
        mode: PutMode::Update(UpdateVersion {
            e_tag: meta.e_tag,
            version: None,
        }),
        ..Default::default()
    };
    store
        .put_opts(&location, new_value.clone().into(), put_opts)
        .await?;
    let data = store.get(&location).await?.bytes().await?;
    assert_eq!(new_value, data);

    let put_opts = object_store::PutOptions {
        mode: PutMode::Update(UpdateVersion {
            e_tag: Some("invalid etag".to_string()),
            version: None,
        }),
        ..Default::default()
    };
    let ret = store
        .put_opts(&location, value.clone().into(), put_opts)
        .await;
    assert!(ret.is_err());
    assert!(matches!(
        ret.unwrap_err(),
        object_store::Error::Precondition { .. }
    ));

    store.delete(&location).await?;
    Ok(())
}
