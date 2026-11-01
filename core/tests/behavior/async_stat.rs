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

use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;
use http::StatusCode;
use reqwest::Url;
use tokio::time::sleep;

use crate::*;

pub fn tests(op: &Operator, tests: &mut Vec<Trial>) {
    let cap = op.info().capability();

    if cap.stat && cap.write {
        tests.extend(async_trials!(
            op,
            test_stat_file,
            test_stat_dir,
            test_stat_nested_parent_dir,
            test_stat_with_special_chars,
            test_stat_not_cleaned_path,
            test_stat_not_exist,
            test_stat_with_if_match,
            test_stat_with_if_none_match,
            test_stat_with_version_conditions,
            test_stat_with_if_modified_since,
            test_stat_with_if_unmodified_since,
            test_stat_with_override_cache_control,
            test_stat_with_override_content_disposition,
            test_stat_with_override_content_type,
            test_stat_root,
            test_stat_with_version,
            stat_with_not_existing_version
        ))
    }

    if cap.stat && !cap.write {
        tests.extend(async_trials!(
            op,
            test_read_only_stat_file_and_dir,
            test_read_only_stat_special_chars,
            test_read_only_stat_not_cleaned_path,
            test_read_only_stat_not_exist,
            test_read_only_stat_with_if_match,
            test_read_only_stat_with_if_none_match,
            test_read_only_stat_root
        ))
    }
}

/// Stat existing file should return metadata
pub async fn test_stat_file(op: Operator) -> Result<()> {
    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content).await.expect("write must succeed");

    let meta = op.stat(&path).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), size as u64);

    // Stat a file with trailing slash should return `NotFound`.
    if op.info().capability().create_dir {
        let result = op.stat(&format!("{path}/")).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::NotFound);
    }

    Ok(())
}

/// Stat existing file should return metadata
pub async fn test_stat_dir(op: Operator) -> Result<()> {
    if !op.info().capability().create_dir {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_dir_path();

    op.create_dir(&path).await.expect("write must succeed");

    let meta = op.stat(&path).await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    // Stat a dir without trailing slash could have two behavior.
    let result = op.stat(path.trim_end_matches('/')).await;
    match result {
        Ok(meta) => assert_eq!(meta.mode(), EntryMode::DIR),
        Err(err) => assert_eq!(err.kind(), ErrorKind::NotFound),
    }

    Ok(())
}

/// Stat the parent dir of existing dir should return metadata
pub async fn test_stat_nested_parent_dir(op: Operator) -> Result<()> {
    if !op.info().capability().create_dir {
        return Ok(());
    }

    let parent = format!("{}", uuid::Uuid::new_v4());
    let file = format!("{}", uuid::Uuid::new_v4());
    let (path, content, _) =
        TEST_FIXTURE.new_file_with_path(op.clone(), &format!("{parent}/{file}"));

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&format!("{parent}/")).await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    Ok(())
}

/// Stat existing file with special chars should return metadata
pub async fn test_stat_with_special_chars(op: Operator) -> Result<()> {
    let path = format!("{} !@#$%^&()_+-=;',.txt", uuid::Uuid::new_v4());
    let (path, content, size) = TEST_FIXTURE.new_file_with_path(op.clone(), &path);

    op.write(&path, content).await.expect("write must succeed");

    let meta = op.stat(&path).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), size as u64);

    Ok(())
}

/// Stat not cleaned path should also succeed.
pub async fn test_stat_not_cleaned_path(op: Operator) -> Result<()> {
    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content).await.expect("write must succeed");

    let meta = op.stat(&format!("//{}", path)).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), size as u64);

    Ok(())
}

/// Stat not exist file should return NotFound
pub async fn test_stat_not_exist(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    // Stat not exist file should return NotFound.
    let meta = op.stat(&path).await;
    assert!(meta.is_err());
    assert_eq!(meta.unwrap_err().kind(), ErrorKind::NotFound);

    // Stat not exist dir should also return NotFound.
    if op.info().capability().create_dir {
        let meta = op.stat(&format!("{path}/")).await;
        assert!(meta.is_err());
        assert_eq!(meta.unwrap_err().kind(), ErrorKind::NotFound);
    }

    Ok(())
}

/// Stat with if_match should succeed, else get a ConditionNotMatch error.
pub async fn test_stat_with_if_match(op: Operator) -> Result<()> {
    if !op.info().capability().stat_with_if_match {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&path).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), size as u64);

    let res = op.stat_with(&path).if_match("\"invalid_etag\"").await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    let result = op
        .stat_with(&path)
        .if_match(meta.etag().expect("etag must exist"))
        .await;
    assert!(result.is_ok());

    let missing = TEST_FIXTURE.new_file_path();
    let err = op
        .stat_with(&missing)
        .if_match(meta.etag().expect("etag must exist"))
        .await
        .expect_err("missing target must fail");
    assert_eq!(err.kind(), ErrorKind::NotFound);

    Ok(())
}

/// Stat with if_none_match should succeed, else get a ConditionNotMatch.
pub async fn test_stat_with_if_none_match(op: Operator) -> Result<()> {
    if !op.info().capability().stat_with_if_none_match {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&path).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), size as u64);

    let res = op
        .stat_with(&path)
        .if_none_match(meta.etag().expect("etag must exist"))
        .await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    let res = op
        .stat_with(&path)
        .if_none_match("\"invalid_etag\"")
        .await?;
    assert_eq!(res.mode(), meta.mode());
    assert_eq!(res.content_length(), meta.content_length());

    let missing = TEST_FIXTURE.new_file_path();
    let err = op
        .stat_with(&missing)
        .if_none_match(meta.etag().expect("etag must exist"))
        .await
        .expect_err("missing target must fail");
    assert_eq!(err.kind(), ErrorKind::NotFound);

    Ok(())
}

/// Version preconditions should compare against the current live object version.
pub async fn test_stat_with_version_conditions(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.stat_with_if_version_match || !cap.stat_with_if_version_not_match {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let (first, _) = gen_bytes(cap);
    let (second, _) = gen_bytes(cap);
    assert_ne!(first, second);

    op.write(&path, first).await?;
    let stale = op
        .stat(&path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();
    op.write(&path, second).await?;
    let current = op
        .stat(&path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();

    op.stat_with(&path).if_version_match(&current).await?;
    let err = op
        .stat_with(&path)
        .if_version_match(&stale)
        .await
        .expect_err("stale version match must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    op.stat_with(&path).if_version_not_match(&stale).await?;
    let err = op
        .stat_with(&path)
        .if_version_not_match(&current)
        .await
        .expect_err("equal version non-match must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    let missing = TEST_FIXTURE.new_file_path();
    for result in [
        op.stat_with(&missing).if_version_match(&current).await,
        op.stat_with(&missing).if_version_not_match(&current).await,
    ] {
        assert_eq!(
            result.expect_err("missing target must fail").kind(),
            ErrorKind::NotFound
        );
    }

    Ok(())
}

/// Stat file with if_modified_since should succeed, otherwise get a ConditionNotMatch error.
pub async fn test_stat_with_if_modified_since(op: Operator) -> Result<()> {
    if !op.info().capability().stat_with_if_modified_since {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&path).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), content.len() as u64);

    let since = meta.last_modified().unwrap() - Duration::from_secs(1);
    let res = op.stat_with(&path).if_modified_since(since).await?;
    assert_eq!(res.last_modified(), meta.last_modified());

    sleep(Duration::from_secs(1)).await;

    let since = meta.last_modified().unwrap() + Duration::from_secs(1);
    let res = op.stat_with(&path).if_modified_since(since).await;
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().kind(), ErrorKind::ConditionNotMatch);

    let missing = TEST_FIXTURE.new_file_path();
    let err = op
        .stat_with(&missing)
        .if_modified_since(since)
        .await
        .expect_err("missing target must fail");
    assert_eq!(err.kind(), ErrorKind::NotFound);

    Ok(())
}

/// Stat file with if_unmodified_since should succeed, otherwise get a ConditionNotMatch error.
pub async fn test_stat_with_if_unmodified_since(op: Operator) -> Result<()> {
    if !op.info().capability().stat_with_if_unmodified_since {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&path).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), content.len() as u64);

    let since = meta.last_modified().unwrap() - Duration::from_secs(1);
    let res = op.stat_with(&path).if_unmodified_since(since).await;
    assert!(res.is_err());
    assert_eq!(res.err().unwrap().kind(), ErrorKind::ConditionNotMatch);

    sleep(Duration::from_secs(1)).await;

    let since = meta.last_modified().unwrap() + Duration::from_secs(1);
    let res = op.stat_with(&path).if_unmodified_since(since).await?;
    assert_eq!(res.last_modified(), meta.last_modified());

    let missing = TEST_FIXTURE.new_file_path();
    let err = op
        .stat_with(&missing)
        .if_unmodified_since(since)
        .await
        .expect_err("missing target must fail");
    assert_eq!(err.kind(), ErrorKind::NotFound);

    Ok(())
}

/// Stat file with override-cache-control should succeed.
pub async fn test_stat_with_override_cache_control(op: Operator) -> Result<()> {
    if !(op.info().capability().stat_with_override_cache_control && op.info().capability().presign)
    {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let target_cache_control = "no-cache, no-store, must-revalidate";
    let signed_req = op
        .presign_stat_with(&path, Duration::from_secs(60))
        .override_cache_control(target_cache_control)
        .await
        .expect("sign must succeed");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }

    let resp = req.send().await.expect("send must succeed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .expect("cache-control header must exist")
            .to_str()
            .expect("cache-control header must be string"),
        target_cache_control
    );

    Ok(())
}

/// Stat file with override_content_disposition should succeed.
pub async fn test_stat_with_override_content_disposition(op: Operator) -> Result<()> {
    if !(op
        .info()
        .capability()
        .stat_with_override_content_disposition
        && op.info().capability().presign)
    {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let target_content_disposition = "attachment; filename=foo.txt";

    let signed_req = op
        .presign_stat_with(&path, Duration::from_secs(60))
        .override_content_disposition(target_content_disposition)
        .await
        .expect("presign must succeed");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }

    let resp = req.send().await.expect("send must succeed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(http::header::CONTENT_DISPOSITION)
            .expect("content-disposition header must exist")
            .to_str()
            .expect("content-disposition header must be string"),
        target_content_disposition
    );

    Ok(())
}

/// Stat file with override_content_type should succeed.
pub async fn test_stat_with_override_content_type(op: Operator) -> Result<()> {
    if !(op.info().capability().stat_with_override_content_type && op.info().capability().presign) {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let target_content_type = "application/opendal";

    let signed_req = op
        .presign_stat_with(&path, Duration::from_secs(60))
        .override_content_type(target_content_type)
        .await
        .expect("presign must succeed");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }

    let resp = req.send().await.expect("send must succeed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(http::header::CONTENT_TYPE)
            .expect("content-type header must exist")
            .to_str()
            .expect("content-type header must be string"),
        target_content_type
    );

    Ok(())
}

/// Root should be able to stat and returns DIR.
pub async fn test_stat_root(op: Operator) -> Result<()> {
    let meta = op.stat("").await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    let meta = op.stat("/").await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    Ok(())
}

/// Stat normal file and dir should return metadata
pub async fn test_read_only_stat_file_and_dir(op: Operator) -> Result<()> {
    let meta = op.stat("normal_file.txt").await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), 30482);

    let meta = op.stat("normal_dir/").await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    Ok(())
}

/// Stat special file and dir should return metadata
pub async fn test_read_only_stat_special_chars(op: Operator) -> Result<()> {
    let meta = op.stat("special_file  !@#$%^&()_+-=;',.txt").await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), 30482);

    let meta = op.stat("special_dir  !@#$%^&()_+-=;',/").await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    Ok(())
}

/// Stat not cleaned path should also succeed.
pub async fn test_read_only_stat_not_cleaned_path(op: Operator) -> Result<()> {
    let meta = op.stat("//normal_file.txt").await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), 30482);

    Ok(())
}

/// Stat not exist file should return NotFound
pub async fn test_read_only_stat_not_exist(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let meta = op.stat(&path).await;
    assert!(meta.is_err());
    assert_eq!(meta.unwrap_err().kind(), ErrorKind::NotFound);

    Ok(())
}

/// Stat with if_match should succeed, else get a ConditionNotMatch error.
pub async fn test_read_only_stat_with_if_match(op: Operator) -> Result<()> {
    if !op.info().capability().stat_with_if_match {
        return Ok(());
    }

    let path = "normal_file.txt";

    let meta = op.stat(path).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), 30482);

    let res = op.stat_with(path).if_match("invalid_etag").await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    let result = op
        .stat_with(path)
        .if_match(meta.etag().expect("etag must exist"))
        .await;
    assert!(result.is_ok());

    Ok(())
}

/// Stat with if_none_match should succeed, else get a ConditionNotMatch.
pub async fn test_read_only_stat_with_if_none_match(op: Operator) -> Result<()> {
    if !op.info().capability().stat_with_if_none_match {
        return Ok(());
    }

    let path = "normal_file.txt";

    let meta = op.stat(path).await?;
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(meta.content_length(), 30482);

    let res = op
        .stat_with(path)
        .if_none_match(meta.etag().expect("etag must exist"))
        .await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    let res = op.stat_with(path).if_none_match("invalid_etag").await?;
    assert_eq!(res.mode(), meta.mode());
    assert_eq!(res.content_length(), meta.content_length());

    Ok(())
}

/// Root should be able to stat and returns DIR.
pub async fn test_read_only_stat_root(op: Operator) -> Result<()> {
    let meta = op.stat("").await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    let meta = op.stat("/").await?;
    assert_eq!(meta.mode(), EntryMode::DIR);

    Ok(())
}

pub async fn test_stat_with_version(op: Operator) -> Result<()> {
    if !op.info().capability().stat_with_version {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(path.as_str(), content.clone())
        .await
        .expect("write must success");
    let first_meta = op.stat(path.as_str()).await.expect("stat must success");
    let first_version = first_meta.version().expect("must have version");

    let first_versioning_meta = op
        .stat_with(path.as_str())
        .version(first_version)
        .await
        .expect("stat must success");
    assert_eq!(first_meta, first_versioning_meta);

    op.write(path.as_str(), content)
        .await
        .expect("write must success");
    let second_meta = op.stat(path.as_str()).await.expect("stat must success");
    let second_version = second_meta.version().expect("must have version");
    assert_ne!(first_version, second_version);

    // we can still `stat` with first_version after writing new data
    let meta = op
        .stat_with(path.as_str())
        .version(first_version)
        .await
        .expect("stat must success");
    assert_eq!(first_meta, meta);

    Ok(())
}

pub async fn stat_with_not_existing_version(op: Operator) -> Result<()> {
    if !op.info().capability().stat_with_version {
        return Ok(());
    }

    // retrieve a valid version
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(path.as_str(), content.clone())
        .await
        .expect("write must success");
    let version = op
        .stat(path.as_str())
        .await
        .expect("stat must success")
        .version()
        .expect("must have version")
        .to_string();

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(path.as_str(), content)
        .await
        .expect("write must success");
    let ret = op.stat_with(path.as_str()).version(version.as_str()).await;
    assert!(ret.is_err());
    assert_eq!(ret.unwrap_err().kind(), ErrorKind::NotFound);

    Ok(())
}
