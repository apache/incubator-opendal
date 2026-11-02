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
use object_store::{GetOptions, GetRange, ObjectStore, ObjectStoreExt};
use object_store_opendal::OpendalStore;

pub fn tests(store: &OpendalStore, tests: &mut Vec<Trial>) {
    tests.push(build_trail("test_basic_get", store, test_basic_get));
    tests.push(build_trail("test_head", store, test_head));
    tests.push(build_trail("test_get_range", store, test_get_range));
    tests.push(build_trail(
        "test_get_opts_with_range",
        store,
        test_get_opts_with_range,
    ));
    tests.push(build_trail(
        "test_get_opts_with_invalid_range",
        store,
        test_get_opts_with_invalid_range,
    ));
    tests.push(build_trail(
        "test_get_opts_with_version",
        store,
        test_get_opts_with_version,
    ));
    tests.push(build_trail(
        "test_get_ops_with_if_match",
        store,
        test_get_ops_with_if_match,
    ));
    tests.push(build_trail(
        "test_get_ops_with_if_none_match",
        store,
        test_get_ops_with_if_none_match,
    ));
    tests.push(build_trail(
        "test_get_opts_with_if_modified_since",
        store,
        test_get_opts_with_if_modified_since,
    ));
    tests.push(build_trail(
        "test_get_opts_with_if_unmodified_since",
        store,
        test_get_opts_with_if_unmodified_since,
    ));
}

pub async fn test_basic_get(store: OpendalStore) -> Result<()> {
    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");

    store.put(&location, value.clone().into()).await?;

    let ret = store.get(&location).await?;

    assert_eq!(0..value.len() as u64, ret.range);
    let data = ret.bytes().await?;
    assert_eq!(value, data);

    let ret = store.get(&"not_exist".into()).await;
    assert!(ret.is_err());
    assert!(matches!(
        ret.err().unwrap(),
        object_store::Error::NotFound { .. }
    ));

    store.delete(&location).await?;
    Ok(())
}

pub async fn test_head(store: OpendalStore) -> Result<()> {
    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");

    store.put(&location, value.clone().into()).await?;

    let meta = store.head(&location).await?;

    assert_eq!(meta.size, value.len() as u64);
    assert_eq!(meta.location, location);

    store.delete(&location).await?;
    Ok(())
}

pub async fn test_get_range(store: OpendalStore) -> Result<()> {
    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");

    store.put(&location, value.clone().into()).await?;

    let ret = store.get_range(&location, 0..5).await?;
    assert_eq!(Bytes::from_static(b"Hello"), ret);

    store.delete(&location).await?;
    Ok(())
}

pub async fn test_get_opts_with_range(store: OpendalStore) -> Result<()> {
    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");

    store.put(&location, value.clone().into()).await?;

    let opts = GetOptions {
        range: Some((0..5).into()),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    let data = ret.bytes().await?;
    assert_eq!(Bytes::from_static(b"Hello"), data);

    let opts = GetOptions {
        range: Some(GetRange::Offset(7)),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    let data = ret.bytes().await?;
    assert_eq!(Bytes::from_static(b"world!"), data);

    let opts = GetOptions {
        range: Some(GetRange::Suffix(6)),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    let data = ret.bytes().await?;
    assert_eq!(Bytes::from_static(b"world!"), data);

    store.delete(&location).await?;
    Ok(())
}

pub async fn test_get_opts_with_invalid_range(store: OpendalStore) -> Result<()> {
    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");

    store.put(&location, value.clone().into()).await?;

    // the end of the range is greater than the size of the object
    let opts = GetOptions {
        range: Some((0..100).into()),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    assert_eq!(ret.range, 0..value.len() as u64);
    assert_eq!(ret.bytes().await?, value);

    // the offset of the range is greater than the size of the object
    let opts = GetOptions {
        range: Some(GetRange::Offset(100)),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    assert_eq!(ret.range, 0..0);

    // the suffix of the range is greater than the size of the object
    let opts = GetOptions {
        range: Some(GetRange::Suffix(100)),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    assert_eq!(ret.range, 0..value.len() as u64);
    assert_eq!(ret.bytes().await?, value);

    store.delete(&location).await?;
    Ok(())
}

pub async fn test_get_opts_with_version(store: OpendalStore) -> Result<()> {
    if !store.info().capability().read_with_version {
        return Ok(());
    }

    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");
    store.put(&location, value.clone().into()).await?;
    let meta = store.head(&location).await?;

    let opts = GetOptions {
        version: meta.version,
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    let data = ret.bytes().await?;
    assert_eq!(value, data);

    let another_location = new_file_path("data").into();
    store
        .put(
            &another_location,
            Bytes::from_static(b"Hello, world!").into(),
        )
        .await?;
    let another_version = store.head(&another_location).await?.version;

    let opts = GetOptions {
        version: another_version,
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await;
    assert!(ret.is_err());
    assert!(matches!(
        ret.err().unwrap(),
        object_store::Error::NotFound { .. }
    ));

    store.delete(&location).await?;
    store.delete(&another_location).await?;
    Ok(())
}

async fn test_get_ops_with_if_match(store: OpendalStore) -> Result<()> {
    if !store.info().capability().read_with_if_match {
        return Ok(());
    }

    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");
    store.put(&location, value.clone().into()).await?;
    let meta = store.head(&location).await?;

    let opts = GetOptions {
        if_match: Some(meta.e_tag.unwrap()),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    let data = ret.bytes().await?;
    assert_eq!(value, data);

    let opts = GetOptions {
        if_match: Some("invalid etag".to_string()),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await;
    assert!(ret.is_err());
    assert!(matches!(
        ret.err().unwrap(),
        object_store::Error::Precondition { .. }
    ));

    store.delete(&location).await?;
    Ok(())
}

async fn test_get_ops_with_if_none_match(store: OpendalStore) -> Result<()> {
    if !store.info().capability().read_with_if_none_match {
        return Ok(());
    }

    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");
    store.put(&location, value.clone().into()).await?;
    let meta = store.head(&location).await?;

    let opts = GetOptions {
        if_none_match: Some("invalid etag".to_string()),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    let data = ret.bytes().await?;
    assert_eq!(value, data);

    let opts = GetOptions {
        if_none_match: Some(meta.e_tag.unwrap()),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await;
    assert!(ret.is_err());
    assert!(matches!(
        ret.err().unwrap(),
        object_store::Error::Precondition { .. }
    ));

    store.delete(&location).await?;
    Ok(())
}

async fn test_get_opts_with_if_modified_since(store: OpendalStore) -> Result<()> {
    if !store.info().capability().read_with_if_modified_since {
        return Ok(());
    }

    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");
    store.put(&location, value.clone().into()).await?;
    let meta = store.head(&location).await?;

    let opts = GetOptions {
        if_modified_since: Some(meta.last_modified - std::time::Duration::from_secs(1)),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    let data = ret.bytes().await?;
    assert_eq!(value, data);

    let opts = GetOptions {
        if_modified_since: Some(meta.last_modified + std::time::Duration::from_secs(1)),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await;
    assert!(ret.is_err());
    assert!(matches!(
        ret.err().unwrap(),
        object_store::Error::Precondition { .. }
    ));

    store.delete(&location).await?;
    Ok(())
}

async fn test_get_opts_with_if_unmodified_since(store: OpendalStore) -> Result<()> {
    if !store.info().capability().read_with_if_unmodified_since {
        return Ok(());
    }

    let location = new_file_path("data").into();
    let value = Bytes::from_static(b"Hello, world!");
    store.put(&location, value.clone().into()).await?;
    let meta = store.head(&location).await?;

    let opts = GetOptions {
        if_unmodified_since: Some(meta.last_modified + std::time::Duration::from_secs(1)),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await?;
    let data = ret.bytes().await?;
    assert_eq!(value, data);

    let opts = GetOptions {
        if_unmodified_since: Some(meta.last_modified - std::time::Duration::from_secs(1)),
        ..Default::default()
    };
    let ret = store.get_opts(&location, opts).await;
    assert!(ret.is_err());
    assert!(matches!(
        ret.err().unwrap(),
        object_store::Error::Precondition { .. }
    ));

    store.delete(&location).await?;
    Ok(())
}
