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

use std::collections::HashMap;
use std::collections::HashSet;

use anyhow::Result;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use futures::TryStreamExt;
use log::debug;

use crate::*;

pub fn tests(op: &Operator, tests: &mut Vec<Trial>) {
    let cap = op.info().full_capability();

    if cap.read && cap.write && cap.list {
        tests.extend(async_trials!(
            op,
            test_check,
            test_list_dir,
            test_list_dir_with_metakey,
            test_list_dir_with_metakey_complete,
            test_list_prefix,
            test_list_rich_dir,
            test_list_empty_dir,
            test_list_non_exist_dir,
            test_list_sub_dir,
            test_list_nested_dir,
            test_list_dir_with_file_path,
            test_list_with_start_after,
            test_list_dir_with_recursive,
            test_list_dir_with_recursive_no_trailing_slash,
            test_list_file_with_recursive,
            test_list_root_with_recursive,
            test_remove_all,
            test_list_files_with_version,
            test_list_with_version_and_limit,
            test_list_with_version_and_start_after
        ))
    }

    if cap.read && !cap.write && cap.list {
        tests.extend(async_trials!(op, test_list_only))
    }
}

/// Check should be OK.
pub async fn test_check(op: Operator) -> Result<()> {
    op.check().await.expect("operator check is ok");

    Ok(())
}

/// List dir should return newly created file.
pub async fn test_list_dir(op: Operator) -> Result<()> {
    let parent = uuid::Uuid::new_v4().to_string();
    let path = format!("{parent}/{}", uuid::Uuid::new_v4());
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes(op.info().full_capability());

    op.write(&path, content).await.expect("write must succeed");

    let mut obs = op.lister(&format!("{parent}/")).await?;
    let mut found = false;
    while let Some(de) = obs.try_next().await? {
        let meta = op.stat(de.path()).await?;
        if de.path() == path {
            assert_eq!(meta.mode(), EntryMode::FILE);

            assert_eq!(meta.content_length(), size as u64);

            found = true
        }
    }
    assert!(found, "file should be found in list");

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

/// List dir with metakey
pub async fn test_list_dir_with_metakey(op: Operator) -> Result<()> {
    let parent = uuid::Uuid::new_v4().to_string();
    let path = format!("{parent}/{}", uuid::Uuid::new_v4());
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes(op.info().full_capability());

    op.write(&path, content).await.expect("write must succeed");

    let mut obs = op
        .lister_with(&format!("{parent}/"))
        .metakey(
            Metakey::Mode
                | Metakey::CacheControl
                | Metakey::ContentDisposition
                | Metakey::ContentLength
                | Metakey::ContentMd5
                | Metakey::ContentRange
                | Metakey::ContentType
                | Metakey::Etag
                | Metakey::LastModified
                | Metakey::Version,
        )
        .await?;
    let mut found = false;
    while let Some(de) = obs.try_next().await? {
        let meta = de.metadata();
        if de.path() == path {
            assert_eq!(meta.mode(), EntryMode::FILE);
            assert_eq!(meta.content_length(), size as u64);

            // We don't care about the value, we just to check there is no panic.
            let _ = meta.cache_control();
            let _ = meta.content_disposition();
            let _ = meta.content_md5();
            let _ = meta.content_range();
            let _ = meta.content_type();
            let _ = meta.etag();
            let _ = meta.last_modified();
            let _ = meta.version();

            found = true
        }
    }
    assert!(found, "file should be found in list");

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

/// List dir with metakey complete
pub async fn test_list_dir_with_metakey_complete(op: Operator) -> Result<()> {
    let parent = uuid::Uuid::new_v4().to_string();
    let path = format!("{parent}/{}", uuid::Uuid::new_v4());
    debug!("Generate a random file: {}", &path);
    let (content, size) = gen_bytes(op.info().full_capability());

    op.write(&path, content).await.expect("write must succeed");

    let mut obs = op
        .lister_with(&format!("{parent}/"))
        .metakey(Metakey::Complete)
        .await?;
    let mut found = false;
    while let Some(de) = obs.try_next().await? {
        let meta = de.metadata();
        if de.path() == path {
            assert_eq!(meta.mode(), EntryMode::FILE);
            assert_eq!(meta.content_length(), size as u64);

            // We don't care about the value, we just to check there is no panic.
            let _ = meta.cache_control();
            let _ = meta.content_disposition();
            let _ = meta.content_md5();
            let _ = meta.content_range();
            let _ = meta.content_type();
            let _ = meta.etag();
            let _ = meta.last_modified();
            let _ = meta.version();

            found = true
        }
    }
    assert!(found, "file should be found in list");

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

/// List prefix should return newly created file.
pub async fn test_list_prefix(op: Operator) -> Result<()> {
    // Dropbox does not support list prefix.
    if op.info().scheme() == Scheme::Dropbox {
        return Ok(());
    }

    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", &path);
    let (content, _) = gen_bytes(op.info().full_capability());

    op.write(&path, content).await.expect("write must succeed");

    let obs = op.list(&path).await?;
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].path(), path);
    assert_eq!(obs[0].metadata().mode(), EntryMode::FILE);

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

/// listing a directory, which contains more objects than a single page can take.
pub async fn test_list_rich_dir(op: Operator) -> Result<()> {
    // Gdrive think that this test is an abuse of their service and redirect us
    // to an infinite loop. Let's ignore this test for gdrive.
    if op.info().scheme() == Scheme::Gdrive {
        return Ok(());
    }

    let parent = "test_list_rich_dir/";
    op.create_dir(parent).await?;

    let mut expected: Vec<String> = (0..=10).map(|num| format!("{parent}file-{num}")).collect();
    for path in expected.iter() {
        op.write(path, "test_list_rich_dir").await?;
    }
    expected.push(parent.to_string());

    let mut objects = op.lister_with(parent).limit(5).await?;
    let mut actual = vec![];
    while let Some(o) = objects.try_next().await? {
        let path = o.path().to_string();
        actual.push(path)
    }
    expected.sort_unstable();
    actual.sort_unstable();

    assert_eq!(actual, expected);

    // List concurrently.
    let mut objects = op
        .lister_with(parent)
        .limit(5)
        .concurrent(5)
        .metakey(Metakey::Complete)
        .await?;
    let mut actual = vec![];
    while let Some(o) = objects.try_next().await? {
        let path = o.path().to_string();
        actual.push(path)
    }
    expected.sort_unstable();
    actual.sort_unstable();

    assert_eq!(actual, expected);

    op.remove_all(parent).await?;
    Ok(())
}

/// List empty dir should return itself.
pub async fn test_list_empty_dir(op: Operator) -> Result<()> {
    let dir = format!("{}/", uuid::Uuid::new_v4());

    op.create_dir(&dir).await.expect("write must succeed");

    // List "dir/" should return "dir/".
    let mut obs = op.lister(&dir).await?;
    let mut objects = HashMap::new();
    while let Some(de) = obs.try_next().await? {
        objects.insert(de.path().to_string(), de);
    }
    assert_eq!(
        objects.len(),
        1,
        "only return the dir itself, but found: {objects:?}"
    );
    assert_eq!(
        objects[&dir].metadata().mode(),
        EntryMode::DIR,
        "given dir should exist and must be dir, but found: {objects:?}"
    );

    // List "dir" should return "dir/".
    let mut obs = op.lister(dir.trim_end_matches('/')).await?;
    let mut objects = HashMap::new();
    while let Some(de) = obs.try_next().await? {
        objects.insert(de.path().to_string(), de);
    }
    assert_eq!(
        objects.len(),
        1,
        "only return the dir itself, but found: {objects:?}"
    );
    assert_eq!(
        objects[&dir].metadata().mode(),
        EntryMode::DIR,
        "given dir should exist and must be dir, but found: {objects:?}"
    );

    // List "dir/" recursively should return "dir/".
    let mut obs = op.lister_with(&dir).recursive(true).await?;
    let mut objects = HashMap::new();
    while let Some(de) = obs.try_next().await? {
        objects.insert(de.path().to_string(), de);
    }
    assert_eq!(
        objects.len(),
        1,
        "only return the dir itself, but found: {objects:?}"
    );
    assert_eq!(
        objects[&dir].metadata().mode(),
        EntryMode::DIR,
        "given dir should exist and must be dir, but found: {objects:?}"
    );

    // List "dir" recursively should return "dir/".
    let mut obs = op
        .lister_with(dir.trim_end_matches('/'))
        .recursive(true)
        .await?;
    let mut objects = HashMap::new();
    while let Some(de) = obs.try_next().await? {
        objects.insert(de.path().to_string(), de);
    }
    assert_eq!(objects.len(), 1, "only return the dir itself");
    assert_eq!(
        objects[&dir].metadata().mode(),
        EntryMode::DIR,
        "given dir should exist and must be dir"
    );

    op.delete(&dir).await.expect("delete must succeed");
    Ok(())
}

/// List non exist dir should return nothing.
pub async fn test_list_non_exist_dir(op: Operator) -> Result<()> {
    let dir = format!("{}/", uuid::Uuid::new_v4());

    let mut obs = op.lister(&dir).await?;
    let mut objects = HashMap::new();
    while let Some(de) = obs.try_next().await? {
        objects.insert(de.path().to_string(), de);
    }
    debug!("got objects: {:?}", objects);

    assert_eq!(objects.len(), 0, "dir should only return empty");
    Ok(())
}

/// List dir should return correct sub dir.
pub async fn test_list_sub_dir(op: Operator) -> Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());

    op.create_dir(&path).await.expect("create must succeed");

    let mut obs = op.lister("/").await?;
    let mut found = false;
    let mut entries = vec![];
    while let Some(de) = obs.try_next().await? {
        if de.path() == path {
            let meta = op.stat(&path).await?;
            assert_eq!(meta.mode(), EntryMode::DIR);
            assert_eq!(de.name(), path);

            found = true
        }
        entries.push(de)
    }
    assert!(
        found,
        "dir should be found in list, but only got: {:?}",
        entries
    );

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

/// List dir should also to list nested dir.
pub async fn test_list_nested_dir(op: Operator) -> Result<()> {
    let parent = format!("{}/", uuid::Uuid::new_v4());
    op.create_dir(&parent)
        .await
        .expect("create dir must succeed");

    let dir = format!("{parent}{}/", uuid::Uuid::new_v4());
    op.create_dir(&dir).await.expect("create must succeed");

    let file_name = uuid::Uuid::new_v4().to_string();
    let file_path = format!("{dir}{file_name}");
    op.write(&file_path, "test_list_nested_dir")
        .await
        .expect("create must succeed");

    let dir_name = format!("{}/", uuid::Uuid::new_v4());
    let dir_path = format!("{dir}{dir_name}");
    op.create_dir(&dir_path).await.expect("create must succeed");

    let obs = op.list(&parent).await?;
    assert_eq!(obs.len(), 2, "parent should got 2 entry");
    let objects: HashMap<&str, &Entry> = obs.iter().map(|e| (e.path(), e)).collect();
    assert_eq!(
        objects
            .get(parent.as_str())
            .expect("parent should be found in list")
            .metadata()
            .mode(),
        EntryMode::DIR
    );
    assert_eq!(
        objects
            .get(dir.as_str())
            .expect("dir should be found in list")
            .metadata()
            .mode(),
        EntryMode::DIR
    );

    let mut obs = op.lister(&dir).await?;
    let mut objects = HashMap::new();

    while let Some(de) = obs.try_next().await? {
        objects.insert(de.path().to_string(), de);
    }
    debug!("got objects: {:?}", objects);

    assert_eq!(objects.len(), 3, "dir should only got 3 objects");

    // Check file
    let meta = op
        .stat(
            objects
                .get(&file_path)
                .expect("file should be found in list")
                .path(),
        )
        .await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), 20);

    // Check dir
    let meta = op
        .stat(
            objects
                .get(&dir_path)
                .expect("file should be found in list")
                .path(),
        )
        .await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    op.delete(&file_path).await.expect("delete must succeed");
    op.delete(&dir_path).await.expect("delete must succeed");
    op.delete(&dir).await.expect("delete must succeed");
    Ok(())
}

/// List with path file should auto add / suffix.
pub async fn test_list_dir_with_file_path(op: Operator) -> Result<()> {
    let parent = uuid::Uuid::new_v4().to_string();
    let file = format!("{parent}/{}", uuid::Uuid::new_v4());

    let (content, _) = gen_bytes(op.info().full_capability());
    op.write(&file, content).await?;

    let obs = op.list(&parent).await?;
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].path(), format!("{parent}/"));
    assert_eq!(obs[0].metadata().mode(), EntryMode::DIR);

    op.delete(&file).await?;

    Ok(())
}

/// List with start after should start listing after the specified key
pub async fn test_list_with_start_after(op: Operator) -> Result<()> {
    if !op.info().full_capability().list_with_start_after {
        return Ok(());
    }

    let dir = &format!("{}/", uuid::Uuid::new_v4());
    op.create_dir(dir).await?;

    let given: Vec<String> = ["file-0", "file-1", "file-2", "file-3", "file-4", "file-5"]
        .iter()
        .map(|name| format!("{dir}{name}-{}", uuid::Uuid::new_v4()))
        .collect();

    given
        .iter()
        .map(|name| async {
            op.write(name, "content")
                .await
                .expect("create must succeed");
        })
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await;

    let mut objects = op.lister_with(dir).start_after(&given[2]).await?;
    let mut actual = vec![];
    while let Some(o) = objects.try_next().await? {
        if o.path() != dir {
            let path = o.path().to_string();
            actual.push(path)
        }
    }

    let expected: Vec<String> = given.into_iter().skip(3).collect();

    assert_eq!(expected, actual);

    op.remove_all(dir).await?;

    Ok(())
}

pub async fn test_list_root_with_recursive(op: Operator) -> Result<()> {
    op.create_dir("/").await?;

    let w = op.lister_with("").recursive(true).await?;
    let actual = w
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .map(|v| v.path().to_string())
        .collect::<HashSet<_>>();

    assert!(actual.contains("/"), "empty root should return itself");
    assert!(!actual.contains(""), "empty root should not return empty");
    Ok(())
}

// Walk top down should output as expected
pub async fn test_list_dir_with_recursive(op: Operator) -> Result<()> {
    let parent = uuid::Uuid::new_v4().to_string();

    let paths = [
        "x/", "x/x/", "x/x/x/", "x/x/x/x/", "x/x/x/y", "x/x/y", "x/y", "x/yy",
    ];
    for path in paths.iter() {
        if path.ends_with('/') {
            op.create_dir(&format!("{parent}/{path}")).await?;
        } else {
            op.write(&format!("{parent}/{path}"), "test_scan").await?;
        }
    }
    let w = op
        .lister_with(&format!("{parent}/x/"))
        .recursive(true)
        .await?;
    let mut actual = w
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .map(|v| {
            v.path()
                .strip_prefix(&format!("{parent}/"))
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    actual.sort();

    let expected = vec![
        "x/", "x/x/", "x/x/x/", "x/x/x/x/", "x/x/x/y", "x/x/y", "x/y", "x/yy",
    ];
    assert_eq!(actual, expected);
    Ok(())
}

// same as test_list_dir_with_recursive except listing 'x' instead of 'x/'
pub async fn test_list_dir_with_recursive_no_trailing_slash(op: Operator) -> Result<()> {
    let parent = uuid::Uuid::new_v4().to_string();

    let paths = [
        "x/", "x/x/", "x/x/x/", "x/x/x/x/", "x/x/x/y", "x/x/y", "x/y", "x/yy",
    ];
    for path in paths.iter() {
        if path.ends_with('/') {
            op.create_dir(&format!("{parent}/{path}")).await?;
        } else {
            op.write(&format!("{parent}/{path}"), "test_scan").await?;
        }
    }
    let w = op
        .lister_with(&format!("{parent}/x"))
        .recursive(true)
        .await?;
    let mut actual = w
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .map(|v| {
            v.path()
                .strip_prefix(&format!("{parent}/"))
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    actual.sort();

    let expected = paths.to_vec();
    assert_eq!(actual, expected);
    Ok(())
}

pub async fn test_list_file_with_recursive(op: Operator) -> Result<()> {
    // Dropbox does not support list file with recursive.
    if op.info().scheme() == Scheme::Dropbox {
        return Ok(());
    }

    let parent = uuid::Uuid::new_v4().to_string();

    let paths = ["y", "yy"];
    for path in paths.iter() {
        if path.ends_with('/') {
            op.create_dir(&format!("{parent}/{path}")).await?;
        } else {
            op.write(&format!("{parent}/{path}"), "test_scan").await?;
        }
    }
    let w = op
        .lister_with(&format!("{parent}/y"))
        .recursive(true)
        .await?;
    let mut actual = w
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .map(|v| {
            v.path()
                .strip_prefix(&format!("{parent}/"))
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    actual.sort();

    let expected = vec!["y", "yy"];
    assert_eq!(actual, expected);
    Ok(())
}

// Remove all should remove all in this path.
pub async fn test_remove_all(op: Operator) -> Result<()> {
    let parent = uuid::Uuid::new_v4().to_string();

    let expected = [
        "x/", "x/y", "x/x/", "x/x/y", "x/x/x/", "x/x/x/y", "x/x/x/x/",
    ];
    for path in expected.iter() {
        if path.ends_with('/') {
            op.create_dir(&format!("{parent}/{path}")).await?;
        } else {
            op.write(&format!("{parent}/{path}"), "test_scan").await?;
        }
    }

    op.remove_all(&format!("{parent}/x/")).await?;

    for path in expected.iter() {
        if path.ends_with('/') {
            continue;
        }
        assert!(
            !op.is_exist(&format!("{parent}/{path}")).await?,
            "{parent}/{path} should be removed"
        )
    }
    Ok(())
}

/// Stat normal file and dir should return metadata
pub async fn test_list_only(op: Operator) -> Result<()> {
    let mut entries = HashMap::new();

    let mut ds = op.lister("/").await?;
    while let Some(de) = ds.try_next().await? {
        entries.insert(de.path().to_string(), op.stat(de.path()).await?.mode());
    }

    assert_eq!(entries["normal_file.txt"], EntryMode::FILE);
    assert_eq!(
        entries["special_file  !@#$%^&()_+-=;',.txt"],
        EntryMode::FILE
    );

    assert_eq!(entries["normal_dir/"], EntryMode::DIR);
    assert_eq!(entries["special_dir  !@#$%^&()_+-=;',/"], EntryMode::DIR);

    Ok(())
}

pub async fn test_list_files_with_version(op: Operator) -> Result<()> {
    if !op.info().full_capability().list_with_version {
        return Ok(());
    }

    let parent = TEST_FIXTURE.new_dir_path();
    let file_name = TEST_FIXTURE.new_file_path();
    let file_path = format!("{}{}", parent, file_name);
    op.write(file_path.as_str(), "1").await?;
    op.write(file_path.as_str(), "2").await?;

    let mut ds = op.list_with(parent.as_str()).version(true).await?;
    ds.retain(|de| de.path() != parent.as_str());

    assert_eq!(ds.len(), 2);
    assert_ne!(ds[0].metadata().version(), ds[1].metadata().version());
    for de in ds {
        assert_eq!(de.name(), file_name);
        let meta = de.metadata();
        assert_eq!(meta.mode(), EntryMode::FILE);

        // just ensure they don't panic
        let _ = meta.content_length();
        let _ = meta.version();
        let _ = meta.last_modified();
        let _ = meta.etag();
        let _ = meta.content_md5();
    }

    Ok(())
}

// listing a directory with version, which contains more object versions than a page can take
pub async fn test_list_with_version_and_limit(op: Operator) -> Result<()> {
    // Gdrive think that this test is an abuse of their service and redirect us
    // to an infinite loop. Let's ignore this test for gdrive.
    if op.info().scheme() == Scheme::Gdrive {
        return Ok(());
    }
    if !op.info().full_capability().list_with_version {
        return Ok(());
    }

    let parent = "test_list_with_version_and_limit/";
    op.create_dir(parent).await?;

    let expected: Vec<String> = (0..=10).map(|num| format!("{parent}file-{num}")).collect();
    for path in expected.iter() {
        // each file has 2 versions
        op.write(path, "1").await?;
        op.write(path, "2").await?;
    }
    let mut expected: Vec<String> = expected
        .into_iter()
        .flat_map(|v| std::iter::repeat(v).take(2))
        .collect();
    expected.push(parent.to_string());

    let mut objects = op.lister_with(parent).version(true).limit(5).await?;
    let mut actual = vec![];
    while let Some(o) = objects.try_next().await? {
        let path = o.path().to_string();
        actual.push(path)
    }
    expected.sort_unstable();
    actual.sort_unstable();

    assert_eq!(actual, expected);

    // List concurrently.
    let mut objects = op
        .lister_with(parent)
        .version(true)
        .limit(5)
        .concurrent(5)
        .metakey(Metakey::Complete)
        .await?;
    let mut actual = vec![];
    while let Some(o) = objects.try_next().await? {
        let path = o.path().to_string();
        actual.push(path)
    }
    expected.sort_unstable();
    actual.sort_unstable();

    assert_eq!(actual, expected);

    op.remove_all(parent).await?;
    Ok(())
}

pub async fn test_list_with_version_and_start_after(op: Operator) -> Result<()> {
    if !op.info().full_capability().list_with_version {
        return Ok(());
    }

    let dir = &format!("{}/", uuid::Uuid::new_v4());

    let given: Vec<String> = ["file-0", "file-1", "file-2", "file-3", "file-4", "file-5"]
        .iter()
        .map(|name| format!("{dir}{name}-{}", uuid::Uuid::new_v4()))
        .collect();

    given
        .iter()
        .map(|name| async {
            op.write(name, "1").await.expect("write must succeed");
            op.write(name, "2").await.expect("write must succeed");
        })
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await;

    let mut objects = op
        .lister_with(dir)
        .version(true)
        .start_after(&given[2])
        .await?;
    let mut actual = vec![];
    while let Some(o) = objects.try_next().await? {
        let path = o.path().to_string();
        actual.push(path)
    }

    let expected: Vec<String> = given.into_iter().skip(3).collect();
    let mut expected: Vec<String> = expected
        .into_iter()
        .flat_map(|v| std::iter::repeat(v).take(2))
        .collect();

    expected.sort_unstable();
    actual.sort_unstable();
    assert_eq!(expected, actual);

    op.remove_all(dir).await?;

    Ok(())
}
