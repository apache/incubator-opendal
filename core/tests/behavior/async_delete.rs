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

use anyhow::Result;
use futures::TryStreamExt;
use opendal::layers::CapabilityOverrideLayer;

use crate::*;

pub fn tests(op: &Operator, tests: &mut Vec<Trial>) {
    let cap = op.info().capability();

    if cap.stat && cap.delete && cap.write {
        tests.extend(async_trials!(
            op,
            test_delete_file,
            test_delete_empty_dir,
            test_delete_with_special_chars,
            test_delete_not_existing,
            test_delete_stream,
            test_remove_one_file,
            test_delete_with_version,
            test_delete_with_not_existing_version,
            test_batch_delete,
            test_batch_delete_with_version
        ));
        if cap.delete_with_recursive {
            tests.extend(async_trials!(op, test_delete_with_recursive_basic));
        }
        if cap.list_with_recursive {
            tests.extend(async_trials!(op, test_remove_all_basic));
            if !cap.create_dir {
                tests.extend(async_trials!(op, test_remove_all_with_prefix_exists));
            }
        }
        if cap.delete_with_if_match {
            tests.extend(async_trials!(
                op,
                test_delete_with_if_match_match,
                test_delete_with_if_match_mismatch,
                test_delete_with_if_match_missing,
                test_batch_delete_with_if_match
            ));
        }
        if cap.delete_with_if_none_match {
            tests.extend(async_trials!(
                op,
                test_delete_with_if_none_match,
                test_batch_delete_with_if_none_match
            ));
        }
        if cap.delete_with_if_version_match {
            tests.extend(async_trials!(op, test_delete_with_if_version_match));
        }
        if cap.delete_with_if_version_not_match {
            tests.extend(async_trials!(op, test_delete_with_if_version_not_match));
        }
        if cap.delete_with_if_match || cap.delete_with_if_version_match {
            tests.extend(async_trials!(
                op,
                test_delete_if_not_changed,
                test_deleter_if_not_changed
            ));
        }
    }
}

/// Delete existing file should succeed.
pub async fn test_delete_file(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content).await.expect("write must succeed");

    op.delete(&path).await?;

    // Stat it again to check.
    assert!(!op.exists(&path).await?);

    Ok(())
}

/// Delete empty dir should succeed.
pub async fn test_delete_empty_dir(op: Operator) -> Result<()> {
    if !op.info().capability().create_dir {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_dir_path();

    op.create_dir(&path).await.expect("create must succeed");

    op.delete(&path).await?;

    Ok(())
}

/// Delete file with special chars should succeed.
pub async fn test_delete_with_special_chars(op: Operator) -> Result<()> {
    let path = format!("{} !@#$%^&()_+-=;',.txt", uuid::Uuid::new_v4());
    let (path, content, _) = TEST_FIXTURE.new_file_with_path(op.clone(), &path);

    op.write(&path, content).await.expect("write must succeed");

    op.delete(&path).await?;

    // Stat it again to check.
    assert!(!op.exists(&path).await?);

    Ok(())
}

/// Delete not existing file should also succeed.
pub async fn test_delete_not_existing(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    op.delete(&path).await?;

    Ok(())
}

/// Remove one file
pub async fn test_remove_one_file(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    op.delete_iter(vec![path.clone()]).await?;

    // Stat it again to check.
    assert!(!op.exists(&path).await?);

    op.write(&format!("/{path}"), content)
        .await
        .expect("write must succeed");

    op.delete_iter(vec![path.clone()]).await?;

    // Stat it again to check.
    assert!(!op.exists(&path).await?);

    Ok(())
}

/// Delete via stream.
pub async fn test_delete_stream(op: Operator) -> Result<()> {
    if !op.info().capability().create_dir {
        return Ok(());
    }
    // Gdrive think that this test is an abuse of their service and redirect us
    // to an infinite loop. Let's ignore this test for gdrive.
    #[cfg(feature = "services-gdrive")]
    if op.info().scheme() == services::GDRIVE_SCHEME {
        return Ok(());
    }

    let dir = uuid::Uuid::new_v4().to_string();
    op.create_dir(&format!("{dir}/"))
        .await
        .expect("create must succeed");

    let expected: Vec<_> = (0..100).collect();
    for path in expected.iter() {
        op.write(&format!("{dir}/{path}"), "delete_stream").await?;
    }

    let mut deleter = op.deleter().await?;
    deleter
        .delete_iter(expected.iter().map(|v| format!("{dir}/{v}")))
        .await?;
    deleter.close().await?;

    // Stat it again to check.
    for path in expected.iter() {
        assert!(
            !op.exists(&format!("{dir}/{path}")).await?,
            "{path} should be removed"
        )
    }

    Ok(())
}

async fn test_blocking_remove_all_with_objects(
    op: Operator,
    parent: String,
    paths: impl IntoIterator<Item = &'static str>,
) -> Result<()> {
    for path in paths {
        let path = format!("{parent}/{path}");
        let (content, _) = gen_bytes(op.info().capability());
        op.write(&path, content).await.expect("write must succeed");
    }

    op.delete_with(&parent).recursive(true).await?;

    let found = op
        .lister_with(&format!("{parent}/"))
        .recursive(true)
        .await
        .expect("list must succeed")
        .try_next()
        .await
        .expect("list must succeed")
        .is_some();

    assert!(!found, "all objects should be removed");

    Ok(())
}

/// Remove all under a prefix
pub async fn test_remove_all_basic(op: Operator) -> Result<()> {
    #[cfg(feature = "services-hf")]
    {
        if op.info().scheme() == services::HF_SCHEME {
            // Hugging Face only guarantees recursive listing for repository trees,
            // while this case expects prefix-recursive semantics for a non-directory path.
            return Ok(());
        }
    }

    let parent = uuid::Uuid::new_v4().to_string();
    test_blocking_remove_all_with_objects(op, parent, ["a/b", "a/c", "a/d/e"]).await
}

/// Remove all under a prefix, while the prefix itself is also an object.
///
/// This test requires flat key storage where a path can be both a file
/// and a directory prefix simultaneously (e.g., S3). Services with real
/// directory semantics (e.g., git-based repos) cannot support this
/// because a path cannot be both a file and a directory.
pub async fn test_remove_all_with_prefix_exists(op: Operator) -> Result<()> {
    #[cfg(feature = "services-hf")]
    {
        if op.info().scheme() == services::HF_SCHEME {
            // Hugging Face does not provide a stable recursive delete contract
            // for repository trees under concurrent commits.
            return Ok(());
        }
    }

    let parent = uuid::Uuid::new_v4().to_string();
    let (content, _) = gen_bytes(op.info().capability());
    op.write(&parent, content)
        .await
        .expect("write must succeed");

    test_blocking_remove_all_with_objects(op, parent, ["a", "a/b", "a/c", "a/b/e"]).await
}

pub async fn test_delete_with_version(op: Operator) -> Result<()> {
    if !op.info().capability().delete_with_version {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(path.as_str(), content)
        .await
        .expect("write must success");
    let meta = op.stat(path.as_str()).await.expect("stat must success");
    let version = meta.version().expect("must have version");

    op.delete(path.as_str()).await.expect("delete must success");
    assert!(!op.exists(path.as_str()).await?);

    // After a simple delete, the data can still be accessed using its version.
    let meta = op
        .stat_with(path.as_str())
        .version(version)
        .await
        .expect("stat must success");
    assert_eq!(version, meta.version().expect("must have version"));

    // After deleting with the version, the data is removed permanently
    op.delete_with(path.as_str())
        .version(version)
        .await
        .expect("delete must success");
    let ret = op.stat_with(path.as_str()).version(version).await;
    assert!(ret.is_err());
    assert_eq!(ret.unwrap_err().kind(), ErrorKind::NotFound);

    Ok(())
}

pub async fn test_delete_with_not_existing_version(op: Operator) -> Result<()> {
    if !op.info().capability().delete_with_version {
        return Ok(());
    }

    // retrieve a valid version
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(path.as_str(), content)
        .await
        .expect("write must success");
    let version = op
        .stat(path.as_str())
        .await
        .expect("stat must success")
        .version()
        .expect("must have stat")
        .to_string();

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(path.as_str(), content)
        .await
        .expect("write must success");
    let ret = op
        .delete_with(path.as_str())
        .version(version.as_str())
        .await;
    assert!(ret.is_ok());

    Ok(())
}

pub async fn test_delete_with_recursive_basic(op: Operator) -> Result<()> {
    if !op.info().capability().delete_with_recursive {
        return Ok(());
    }

    #[cfg(feature = "services-hf")]
    {
        if op.info().scheme() == services::HF_SCHEME {
            // Hugging Face does not provide a stable recursive delete contract
            // for repository trees under concurrent commits.
            return Ok(());
        }
    }

    let base = format!("delete_recursive_{}/", uuid::Uuid::new_v4());

    let files = [
        format!("{base}file.txt"),
        format!("{base}dir1/file1.txt"),
        format!("{base}dir1/dir2/file2.txt"),
    ];

    for path in &files {
        op.write(path, "delete recursive").await?;
    }

    op.delete_with(&base).recursive(true).await?;

    let mut l = op.lister_with(&base).recursive(true).await?;
    assert!(
        l.try_next().await?.is_none(),
        "all entries should be removed"
    );

    for path in &files {
        assert!(!op.exists(path).await?, "{path} should be removed");
    }

    Ok(())
}

pub async fn test_batch_delete(op: Operator) -> Result<()> {
    let mut cap = op.info().capability();
    if cap.delete_max_size.unwrap_or(1) <= 1 {
        return Ok(());
    }

    cap.delete_max_size = Some(2);
    let op = op.layer(CapabilityOverrideLayer::new(move |_| cap));

    let mut files = Vec::new();
    for _ in 0..5 {
        let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
        op.write(path.as_str(), content)
            .await
            .expect("write must succeed");
        files.push(path);
    }

    op.delete_iter(files.clone())
        .await
        .expect("batch delete must succeed");

    for path in files {
        let stat = op.stat(path.as_str()).await;
        assert!(stat.is_err());
        assert_eq!(stat.unwrap_err().kind(), ErrorKind::NotFound);
    }

    Ok(())
}

pub async fn test_batch_delete_with_version(op: Operator) -> Result<()> {
    let mut cap = op.info().capability();
    if !cap.delete_with_version {
        return Ok(());
    }
    if cap.delete_max_size.unwrap_or(1) <= 1 {
        return Ok(());
    }

    cap.delete_max_size = Some(2);
    let op = op.layer(CapabilityOverrideLayer::new(move |_| cap));

    let mut files = Vec::new();
    for _ in 0..5 {
        let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
        op.write(path.as_str(), content)
            .await
            .expect("write must succeed");
        let meta = op.stat(path.as_str()).await.expect("stat must succeed");
        let version = meta.version().expect("must have version");
        let op_args = options::DeleteOptions {
            version: Some(version.to_owned()),
            ..Default::default()
        };
        files.push((path, op_args));
    }

    op.delete_iter(files.clone())
        .await
        .expect("batch delete must succeed");

    for (path, args) in files {
        let stat = op
            .stat_with(path.as_str())
            .version(args.version.as_deref().unwrap())
            .await;
        assert!(stat.is_err());
        assert_eq!(stat.unwrap_err().kind(), ErrorKind::NotFound);
    }

    Ok(())
}

/// Delete with a matching `If-Match` ETag should succeed and remove the object.
pub async fn test_delete_with_if_match_match(op: Operator) -> Result<()> {
    if !op.info().capability().delete_with_if_match {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content).await.expect("write must succeed");

    let meta = op.stat(&path).await.expect("stat must succeed");
    let etag = meta.etag().expect("etag must be present");

    op.delete_with(&path).if_match(etag).await?;

    assert!(!op.exists(&path).await?);

    Ok(())
}

/// Delete with a stale `If-Match` ETag should fail with
/// [`ErrorKind::ConditionNotMatch`] and leave the replacement intact.
pub async fn test_delete_with_if_match_mismatch(op: Operator) -> Result<()> {
    if !op.info().capability().delete_with_if_match {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content).await.expect("write must succeed");

    let stale_etag = op
        .stat(&path)
        .await
        .expect("stat must succeed")
        .etag()
        .expect("etag must be present")
        .to_string();
    let replacement = "replacement generation";
    op.write(&path, replacement)
        .await
        .expect("replacement write must succeed");

    let err = op
        .delete_with(&path)
        .if_match(&stale_etag)
        .await
        .expect_err("stale ETag must not delete the replacement");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    assert_eq!(op.read(&path).await?.to_bytes(), replacement.as_bytes());

    op.delete(&path).await?;

    Ok(())
}

/// Batch delete should apply each entry's `If-Match` condition independently.
pub async fn test_batch_delete_with_if_match(op: Operator) -> Result<()> {
    let mut cap = op.info().capability();
    if cap.delete_max_size.unwrap_or(1) <= 1 {
        return Ok(());
    }

    cap.delete_max_size = Some(2);
    let op = op.layer(CapabilityOverrideLayer::new(move |_| cap));

    let (matching_path, matching_content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&matching_path, matching_content)
        .await
        .expect("write must succeed");
    let matching_etag = op
        .stat(&matching_path)
        .await
        .expect("stat must succeed")
        .etag()
        .expect("etag must be present")
        .to_string();

    let (stale_path, stale_content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&stale_path, stale_content)
        .await
        .expect("write must succeed");
    let stale_etag = op
        .stat(&stale_path)
        .await
        .expect("stat must succeed")
        .etag()
        .expect("etag must be present")
        .to_string();
    let replacement = "replacement generation";
    op.write(&stale_path, replacement)
        .await
        .expect("replacement write must succeed");

    let err = op
        .delete_iter([
            (
                matching_path.clone(),
                options::DeleteOptions {
                    if_match: Some(matching_etag.clone()),
                    ..Default::default()
                },
            ),
            (
                stale_path.clone(),
                options::DeleteOptions {
                    if_match: Some(stale_etag.clone()),
                    ..Default::default()
                },
            ),
        ])
        .await
        .expect_err("batch delete must report the stale ETag");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    assert!(!op.exists(&matching_path).await?);
    assert_eq!(
        op.read(&stale_path).await?.to_bytes(),
        replacement.as_bytes()
    );

    op.delete(&stale_path).await?;

    Ok(())
}

/// A matching delete condition requires a live target.
pub async fn test_delete_with_if_match_missing(op: Operator) -> Result<()> {
    let path = TEST_FIXTURE.new_file_path();
    let err = op
        .delete_with(&path)
        .if_match("\"missing-etag\"")
        .await
        .expect_err("missing target must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    Ok(())
}

/// Delete with `If-None-Match` should reject equality and accept inequality or absence.
pub async fn test_delete_with_if_none_match(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content).await?;
    let etag = op
        .stat(&path)
        .await?
        .etag()
        .expect("etag must exist")
        .to_string();

    let err = op
        .delete_with(&path)
        .if_none_match(&etag)
        .await
        .expect_err("equal ETag non-match must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    assert!(op.exists(&path).await?);

    op.delete_with(&path)
        .if_none_match("\"different-etag\"")
        .await?;
    op.delete_with(&path).if_none_match(&etag).await?;

    Ok(())
}

/// Batch delete should preserve a successful non-match condition on a missing target.
pub async fn test_batch_delete_with_if_none_match(op: Operator) -> Result<()> {
    let mut cap = op.info().capability();
    if cap.delete_max_size.unwrap_or(1) <= 1 {
        return Ok(());
    }

    cap.delete_max_size = Some(2);
    let op = op.layer(CapabilityOverrideLayer::new(move |_| cap));

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content).await?;
    let missing_path = TEST_FIXTURE.new_file_path();

    op.delete_iter([
        (
            path.clone(),
            options::DeleteOptions {
                if_none_match: Some("\"different-etag\"".to_owned()),
                ..Default::default()
            },
        ),
        (
            missing_path,
            options::DeleteOptions {
                if_none_match: Some("\"different-etag\"".to_owned()),
                ..Default::default()
            },
        ),
    ])
    .await?;
    assert!(!op.exists(&path).await?);

    Ok(())
}

/// Version match should require equality with the current live target version.
pub async fn test_delete_with_if_version_match(op: Operator) -> Result<()> {
    let cap = op.info().capability();
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
    op.write(&path, second.clone()).await?;
    let current = op
        .stat(&path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();

    let err = op
        .delete_with(&path)
        .if_version_match(&stale)
        .await
        .expect_err("stale version match must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    op.delete_with(&path).if_version_match(&current).await?;

    let err = op
        .delete_with(&path)
        .if_version_match(&current)
        .await
        .expect_err("version match requires a live target");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Version non-match should accept a different or missing live target version.
pub async fn test_delete_with_if_version_not_match(op: Operator) -> Result<()> {
    let cap = op.info().capability();
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

    let err = op
        .delete_with(&path)
        .if_version_not_match(&current)
        .await
        .expect_err("equal version non-match must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    assert!(op.exists(&path).await?);

    op.delete_with(&path).if_version_not_match(&stale).await?;
    op.delete_with(&path).if_version_not_match(&current).await?;

    Ok(())
}

/// `if_not_changed` should reject stale metadata and accept the current state.
pub async fn test_delete_if_not_changed(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    let path = TEST_FIXTURE.new_file_path();
    let (first, _) = gen_bytes(cap);
    let (second, _) = gen_bytes(cap);
    assert_ne!(first, second);

    op.write(&path, first).await?;
    let stale = op.stat(&path).await?;
    op.write(&path, second).await?;

    let err = op
        .delete_with(&path)
        .if_not_changed(&stale)
        .await
        .expect_err("stale metadata must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    let current = op.stat(&path).await?;
    op.delete_with(&path).if_not_changed(&current).await?;

    let err = op
        .delete_with(&path)
        .if_not_changed(&current)
        .await
        .expect_err("unchanged metadata requires a live target");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// `Deleter` should lower `if_not_changed` before passing a delete to the raw queue.
pub async fn test_deleter_if_not_changed(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content).await?;
    let expected = op.stat(&path).await?;

    let mut deleter = op.deleter().await?;
    deleter
        .delete((
            path.clone(),
            options::DeleteOptions {
                if_not_changed: Some(expected),
                ..Default::default()
            },
        ))
        .await?;
    deleter.close().await?;
    assert!(!op.exists(&path).await?);

    Ok(())
}
