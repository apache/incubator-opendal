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

use crate::*;

pub fn tests(op: &Operator, tests: &mut Vec<Trial>) {
    let cap = op.info().capability();

    if cap.read && cap.write && cap.copy {
        tests.extend(async_trials!(
            op,
            test_copy_file_with_ascii_name,
            test_copy_file_with_non_ascii_name,
            test_copy_non_existing_source,
            test_copy_source_dir,
            test_copy_target_dir,
            test_copy_self,
            test_copy_nested,
            test_copy_overwrite,
            test_copier_file,
            test_copier_completion,
            test_copier_abort
        ))
    }

    if cap.read && cap.write && cap.copy && cap.copy_can_multi {
        tests.extend(async_trials!(
            op,
            test_copy_with_chunk,
            test_copy_with_chunk_and_concurrent
        ))
    }

    if cap.read && cap.write && cap.copy && cap.copy_with_if_not_exists {
        tests.extend(async_trials!(
            op,
            test_copy_with_if_not_exists_to_new_file,
            test_copy_with_if_not_exists_to_existing_file,
            test_copier_with_if_not_exists_to_new_file,
            test_copier_with_if_not_exists_to_existing_file
        ))
    }

    if cap.read && cap.write && cap.copy && cap.copy_with_if_match {
        tests.extend(async_trials!(
            op,
            test_copy_with_if_match_match,
            test_copy_with_if_match_mismatch
        ))
    }

    if cap.read && cap.write && cap.copy && cap.copy_with_if_none_match {
        tests.extend(async_trials!(op, test_copy_with_if_none_match))
    }

    if cap.read
        && cap.write
        && cap.copy
        && cap.copy_with_if_version_match
        && cap.copy_with_if_version_not_match
    {
        tests.extend(async_trials!(op, test_copy_with_version_conditions))
    }

    if cap.read
        && cap.write
        && cap.stat
        && cap.copy
        && (cap.copy_with_if_match || cap.copy_with_if_version_match)
    {
        tests.extend(async_trials!(op, test_copy_if_not_changed))
    }

    if cap.read && cap.write && cap.stat && cap.copy && cap.copy_with_source_version {
        tests.extend(async_trials!(
            op,
            test_copy_with_source_version_to_new_file,
            test_copy_with_source_version_to_same_file
        ))
    }
}

fn copy_multi_chunk_size(cap: Capability) -> Option<(usize, usize)> {
    let chunk = cap
        .copy_multi_min_size
        .expect("copy_can_multi should set copy_multi_min_size");
    let max_chunk = cap
        .copy_multi_max_size
        .expect("copy_can_multi should set copy_multi_max_size");
    assert!(
        chunk <= max_chunk,
        "copy_multi_min_size should not exceed copy_multi_max_size"
    );

    let source_size = chunk.checked_add(1)?;

    if cap
        .write_total_max_size
        .is_some_and(|max_size| source_size > max_size)
    {
        return None;
    }

    Some((chunk, source_size))
}

/// Copy a file with ascii name and test contents.
pub async fn test_copy_file_with_ascii_name(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();

    let metadata = op.copy(&source_path, &target_path).await?;
    if metadata.is_file() {
        assert_eq!(metadata.content_length(), source_content.len() as u64);
    }

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy a file with non ascii name and test contents.
pub async fn test_copy_file_with_non_ascii_name(op: Operator) -> Result<()> {
    // Koofr does not support non-ascii name.(https://github.com/apache/opendal/issues/4051)
    #[cfg(feature = "services-koofr")]
    if op.info().scheme() == services::KOOFR_SCHEME {
        return Ok(());
    }

    let source_path = "🐂🍺中文.docx";
    let target_path = "😈🐅Français.docx";
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(source_path, source_content.clone()).await?;
    op.copy(source_path, target_path).await?;

    let target_content = op
        .read(target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(source_path).await.expect("delete must succeed");
    op.delete(target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy a nonexistent source should return an error.
pub async fn test_copy_non_existing_source(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let target_path = uuid::Uuid::new_v4().to_string();

    let err = op
        .copy(&source_path, &target_path)
        .await
        .expect_err("copy must fail");
    assert_eq!(err.kind(), ErrorKind::NotFound);
    Ok(())
}

/// Copy a dir as source should return an error.
pub async fn test_copy_source_dir(op: Operator) -> Result<()> {
    if !op.info().capability().create_dir {
        return Ok(());
    }

    let source_path = format!("{}/", uuid::Uuid::new_v4());
    let target_path = uuid::Uuid::new_v4().to_string();

    op.create_dir(&source_path).await?;

    let err = op
        .copy(&source_path, &target_path)
        .await
        .expect_err("copy must fail");
    assert_eq!(err.kind(), ErrorKind::IsADirectory);
    Ok(())
}

/// Copy to a dir should return an error.
pub async fn test_copy_target_dir(op: Operator) -> Result<()> {
    if !op.info().capability().create_dir {
        return Ok(());
    }

    let source_path = uuid::Uuid::new_v4().to_string();
    let (content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, content).await?;

    let target_path = format!("{}/", uuid::Uuid::new_v4());

    op.create_dir(&target_path).await?;

    let err = op
        .copy(&source_path, &target_path)
        .await
        .expect_err("copy must fail");
    assert_eq!(err.kind(), ErrorKind::IsADirectory);

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy a file to self should return an error.
pub async fn test_copy_self(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, content).await?;

    let err = op
        .copy(&source_path, &source_path)
        .await
        .expect_err("copy must fail");
    assert_eq!(err.kind(), ErrorKind::IsSameFile);

    op.delete(&source_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy to a nested path, parent path should be created successfully.
pub async fn test_copy_nested(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content.clone()).await?;

    let target_path = format!(
        "{}/{}/{}",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4()
    );

    op.copy(&source_path, &target_path).await?;

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy to a exist path should overwrite successfully.
pub async fn test_copy_overwrite(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let (target_content, _) = gen_bytes(op.info().capability());
    assert_ne!(source_content, target_content);

    op.write(&target_path, target_content).await?;

    op.copy(&source_path, &target_path).await?;

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy with if_not_exists to a new file should succeed.
pub async fn test_copy_with_if_not_exists_to_new_file(op: Operator) -> Result<()> {
    if !op.info().capability().copy_with_if_not_exists {
        return Ok(());
    }

    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();

    // Copy with if_not_exists to a non-existing file should succeed
    op.copy_with(&source_path, &target_path)
        .if_not_exists(true)
        .await?;

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy with if_not_exists to an existing file should fail.
pub async fn test_copy_with_if_not_exists_to_existing_file(op: Operator) -> Result<()> {
    if !op.info().capability().copy_with_if_not_exists {
        return Ok(());
    }

    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let (target_content, _) = gen_bytes(op.info().capability());
    assert_ne!(source_content, target_content);

    // Write to target file first
    op.write(&target_path, target_content.clone()).await?;

    // Copy with if_not_exists to an existing file should fail
    let err = op
        .copy_with(&source_path, &target_path)
        .if_not_exists(true)
        .await
        .expect_err("copy must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    // Verify target file content is unchanged
    let current_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(current_content),
        sha256_digest(&target_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy with if_match matching the destination ETag should overwrite it.
pub async fn test_copy_with_if_match_match(op: Operator) -> Result<()> {
    if !op.info().capability().copy_with_if_match {
        return Ok(());
    }

    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());
    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let (target_content, _) = gen_bytes(op.info().capability());
    assert_ne!(source_content, target_content);
    op.write(&target_path, target_content.clone()).await?;

    let Some(etag) = op.stat(&target_path).await?.etag().map(|s| s.to_string()) else {
        op.delete(&source_path).await.expect("delete must succeed");
        op.delete(&target_path).await.expect("delete must succeed");
        return Ok(());
    };

    op.copy_with(&source_path, &target_path)
        .if_match(&etag)
        .await?;

    let current_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(current_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy with if_match not matching the destination ETag should fail.
pub async fn test_copy_with_if_match_mismatch(op: Operator) -> Result<()> {
    if !op.info().capability().copy_with_if_match {
        return Ok(());
    }

    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());
    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let (target_content, _) = gen_bytes(op.info().capability());
    assert_ne!(source_content, target_content);
    op.write(&target_path, target_content.clone()).await?;

    let err = op
        .copy_with(&source_path, &target_path)
        .if_match("\"00000000000000000000000000000000\"")
        .await
        .expect_err("copy must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    let current_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(current_content),
        sha256_digest(&target_content),
    );

    let missing_path = uuid::Uuid::new_v4().to_string();
    let err = op
        .copy_with(&source_path, &missing_path)
        .if_match("\"00000000000000000000000000000000\"")
        .await
        .expect_err("missing destination must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy with `If-None-Match` should reject equality and accept inequality or absence.
pub async fn test_copy_with_if_none_match(op: Operator) -> Result<()> {
    let source_path = TEST_FIXTURE.new_file_path();
    let target_path = TEST_FIXTURE.new_file_path();
    let missing_path = TEST_FIXTURE.new_file_path();
    let (source_content, _) = gen_bytes(op.info().capability());
    let (target_content, _) = gen_bytes(op.info().capability());
    assert_ne!(source_content, target_content);

    op.write(&source_path, source_content.clone()).await?;
    op.write(&target_path, target_content).await?;
    let etag = op
        .stat(&target_path)
        .await?
        .etag()
        .expect("etag must exist")
        .to_string();

    let err = op
        .copy_with(&source_path, &target_path)
        .if_none_match(&etag)
        .await
        .expect_err("equal ETag non-match must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    op.copy_with(&source_path, &target_path)
        .if_none_match("\"different-etag\"")
        .await?;
    op.copy_with(&source_path, &missing_path)
        .if_none_match(&etag)
        .await?;

    assert_eq!(op.read(&target_path).await?.to_bytes(), source_content);
    Ok(())
}

/// Version preconditions should compare against the current destination version.
pub async fn test_copy_with_version_conditions(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    let source_path = TEST_FIXTURE.new_file_path();
    let target_path = TEST_FIXTURE.new_file_path();
    let missing_path = TEST_FIXTURE.new_file_path();
    let (source_content, _) = gen_bytes(cap);
    let (first_target, _) = gen_bytes(cap);
    let (second_target, _) = gen_bytes(cap);
    assert_ne!(source_content, first_target);
    assert_ne!(source_content, second_target);
    assert_ne!(first_target, second_target);

    op.write(&source_path, source_content).await?;
    op.write(&target_path, first_target).await?;
    let stale = op
        .stat(&target_path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();
    op.write(&target_path, second_target).await?;
    let current = op
        .stat(&target_path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();

    let err = op
        .copy_with(&source_path, &target_path)
        .if_version_match(&stale)
        .await
        .expect_err("stale destination version must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    let err = op
        .copy_with(&source_path, &target_path)
        .if_version_not_match(&current)
        .await
        .expect_err("equal destination version must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    op.copy_with(&source_path, &target_path)
        .if_version_match(&current)
        .await?;
    op.copy_with(&source_path, &target_path)
        .if_version_not_match(&current)
        .await?;

    for result in [
        op.copy_with(&source_path, &missing_path)
            .if_version_match(&current)
            .await,
        op.copy_with(&source_path, &missing_path)
            .if_version_not_match(&current)
            .await,
    ] {
        assert_eq!(
            result.expect_err("missing destination must fail").kind(),
            ErrorKind::ConditionNotMatch
        );
    }

    Ok(())
}

/// `if_not_changed` should guard the destination against a stale observation.
pub async fn test_copy_if_not_changed(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    let source_path = TEST_FIXTURE.new_file_path();
    let target_path = TEST_FIXTURE.new_file_path();
    let (source_content, _) = gen_bytes(cap);
    let (target_content, _) = gen_bytes(cap);
    assert_ne!(source_content, target_content);

    op.write(&source_path, source_content).await?;
    op.write(&target_path, target_content).await?;
    let expected = op.stat(&target_path).await?;

    op.copy_with(&source_path, &target_path)
        .if_not_changed(&expected)
        .await?;
    let err = op
        .copy_with(&source_path, &target_path)
        .if_not_changed(&expected)
        .await
        .expect_err("stale destination metadata must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Copy with source_version should copy a specific source version to a new file.
pub async fn test_copy_with_source_version_to_new_file(op: Operator) -> Result<()> {
    if !op.info().capability().copy_with_source_version {
        return Ok(());
    }

    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());
    let (new_content, _) = gen_bytes(op.info().capability());
    assert_ne!(source_content, new_content);

    op.write(&source_path, source_content.clone()).await?;
    let version = op
        .stat(&source_path)
        .await?
        .version()
        .expect("must have version")
        .to_string();

    op.write(&source_path, new_content).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    op.copy_with(&source_path, &target_path)
        .source_version(version)
        .await?;

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy with source_version should allow copying a specific source version to itself.
pub async fn test_copy_with_source_version_to_same_file(op: Operator) -> Result<()> {
    if !op.info().capability().copy_with_source_version {
        return Ok(());
    }

    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());
    let (new_content, _) = gen_bytes(op.info().capability());
    assert_ne!(source_content, new_content);

    op.write(&source_path, source_content.clone()).await?;
    let version = op
        .stat(&source_path)
        .await?
        .version()
        .expect("must have version")
        .to_string();

    op.write(&source_path, new_content).await?;

    op.copy_with(&source_path, &source_path)
        .source_version(version)
        .await?;

    let current_content = op
        .read(&source_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(current_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy with chunk should copy a file successfully.
pub async fn test_copy_with_chunk(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    let Some((chunk, source_size)) = copy_multi_chunk_size(cap) else {
        return Ok(());
    };

    let source_path = uuid::Uuid::new_v4().to_string();
    let source_content = gen_fixed_bytes(source_size);

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    op.copy_with(&source_path, &target_path)
        .chunk(chunk)
        .await?;

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(target_content.len(), source_size, "read size");
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copier should copy a file and commit the target after completion.
pub async fn test_copier_file(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let mut copier = op.copier(&source_path, &target_path).await?;

    while copier.next().await?.is_some() {}

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy with chunk and concurrent should copy a file successfully.
pub async fn test_copy_with_chunk_and_concurrent(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    let Some((chunk, source_size)) = copy_multi_chunk_size(cap) else {
        return Ok(());
    };

    let source_path = uuid::Uuid::new_v4().to_string();
    let source_content = gen_fixed_bytes(source_size);

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    op.copy_with(&source_path, &target_path)
        .chunk(chunk)
        .concurrent(4)
        .await?;

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(target_content.len(), source_size, "read size");
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Completed copier should keep returning `None`.
pub async fn test_copier_completion(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let mut copier = op.copier(&source_path, &target_path).await?;

    while copier.next().await?.is_some() {}
    assert!(copier.next().await?.is_none());

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Aborting a copier should be explicit and idempotent for completed one-shot copies.
pub async fn test_copier_abort(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let mut copier = op.copier(&source_path, &target_path).await?;
    copier.abort().await?;

    op.delete(&source_path).await.expect("delete must succeed");
    let _ = op.delete(&target_path).await;
    Ok(())
}

/// Copier with if_not_exists to a new file should succeed.
pub async fn test_copier_with_if_not_exists_to_new_file(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let mut copier = op
        .copier_with(&source_path, &target_path)
        .if_not_exists(true)
        .await?;

    while copier.next().await?.is_some() {}

    let target_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(target_content),
        sha256_digest(&source_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copier with if_not_exists to an existing file should fail.
pub async fn test_copier_with_if_not_exists_to_existing_file(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes(op.info().capability());

    op.write(&source_path, source_content).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let (target_content, _) = gen_bytes(op.info().capability());
    op.write(&target_path, target_content.clone()).await?;

    let mut copier = op
        .copier_with(&source_path, &target_path)
        .if_not_exists(true)
        .await
        .expect("copier must be created");
    let err = copier.next().await.expect_err("copier must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    let current_content = op
        .read(&target_path)
        .await
        .expect("read must succeed")
        .to_bytes();
    assert_eq!(
        sha256_digest(current_content),
        sha256_digest(&target_content),
    );

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}
