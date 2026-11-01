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

    if cap.restore && cap.read && cap.write && cap.delete {
        tests.extend(async_trials!(
            op,
            test_restore_deleted_file,
            test_restore_repeated_delete,
            test_restore_live_file,
            test_restore_not_found,
            test_restore_not_found_with_matching_prefix
        ));
    }

    if cap.restore && cap.restore_with_version && cap.read && cap.write && cap.stat_with_version {
        tests.extend(async_trials!(op, test_restore_with_version));
    }

    if cap.restore
        && cap.restore_with_version
        && cap.restore_with_if_not_exists
        && cap.read
        && cap.write
        && cap.delete
        && cap.stat_with_version
    {
        tests.extend(async_trials!(
            op,
            test_restore_with_if_not_exists,
            test_restore_with_if_not_exists_conflict
        ));
    }
}

pub async fn test_restore_deleted_file(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content.clone()).await?;
    op.delete(&path).await?;

    assert!(!op.exists(&path).await?);
    op.restore(&path).await?;

    assert_eq!(op.read(&path).await?.to_bytes(), content);
    Ok(())
}

pub async fn test_restore_repeated_delete(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content.clone()).await?;
    op.delete(&path).await?;
    op.delete(&path).await?;

    op.restore(&path).await?;
    op.restore(&path).await?;
    assert_eq!(op.read(&path).await?.to_bytes(), content);
    Ok(())
}

pub async fn test_restore_live_file(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content.clone()).await?;

    op.restore(&path).await?;

    assert_eq!(op.read(&path).await?.to_bytes(), content);
    Ok(())
}

pub async fn test_restore_not_found(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    let err = op
        .restore(&path)
        .await
        .expect_err("restoring an unknown path must fail");
    assert_eq!(err.kind(), ErrorKind::NotFound);
    Ok(())
}

pub async fn test_restore_with_version(op: Operator) -> Result<()> {
    let (path, old_content, _) = TEST_FIXTURE.new_file(op.clone());
    let (new_content, _) = gen_bytes(op.info().capability());
    assert_ne!(old_content, new_content);

    op.write(&path, old_content.clone()).await?;
    let version = op
        .stat(&path)
        .await?
        .version()
        .expect("version must be present")
        .to_string();
    op.write(&path, new_content).await?;

    op.restore_with(&path).version(version).await?;

    assert_eq!(op.read(&path).await?.to_bytes(), old_content);
    Ok(())
}

pub async fn test_restore_with_if_not_exists(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content.clone()).await?;
    let version = op
        .stat(&path)
        .await?
        .version()
        .expect("version must be present")
        .to_string();
    op.delete(&path).await?;

    op.restore_with(&path)
        .version(version)
        .if_not_exists(true)
        .await?;

    assert_eq!(op.read(&path).await?.to_bytes(), content);
    Ok(())
}

pub async fn test_restore_with_if_not_exists_conflict(op: Operator) -> Result<()> {
    let (path, old_content, _) = TEST_FIXTURE.new_file(op.clone());
    let (new_content, _) = gen_bytes(op.info().capability());
    assert_ne!(old_content, new_content);

    op.write(&path, old_content).await?;
    let version = op
        .stat(&path)
        .await?
        .version()
        .expect("version must be present")
        .to_string();
    op.write(&path, new_content.clone()).await?;

    let err = op
        .restore_with(&path)
        .version(version)
        .if_not_exists(true)
        .await
        .expect_err("conditional restore must not overwrite a live path");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    assert_eq!(op.read(&path).await?.to_bytes(), new_content);
    Ok(())
}

/// A prefix match must not restore or otherwise change a different object.
pub async fn test_restore_not_found_with_matching_prefix(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    op.write(&path, content.clone()).await?;
    op.delete(&path).await?;

    let prefix = &path[..path.len() - 1];
    let err = op
        .restore(prefix)
        .await
        .expect_err("a matching prefix is not a recoverable object");
    assert_eq!(err.kind(), ErrorKind::NotFound);
    assert!(!op.exists(&path).await?);

    op.restore(&path).await?;
    assert_eq!(op.read(&path).await?.to_bytes(), content);
    Ok(())
}
