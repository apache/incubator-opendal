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

pub fn behavior_copy_tests(op: &Operator) -> Vec<Trial> {
    let cap = op.info().full_capability();

    if !(cap.read && cap.write && cap.copy) {
        return vec![];
    }

    async_trials!(
        op,
        test_copy_file_with_ascii_name,
        test_copy_file_with_non_ascii_name,
        test_copy_non_existing_source,
        test_copy_source_dir,
        test_copy_target_dir,
        test_copy_self,
        test_copy_nested,
        test_copy_overwrite
    )
}

/// Copy a file with ascii name and test contents.
pub async fn test_copy_file_with_ascii_name(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes();

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();

    op.copy(&source_path, &target_path).await?;

    let target_content = op.read(&target_path).await.expect("read must succeed");
    assert_eq!(target_content, source_content);

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy a file with non ascii name and test contents.
pub async fn test_copy_file_with_non_ascii_name(op: Operator) -> Result<()> {
    let source_path = "🐂🍺中文.docx";
    let target_path = "😈🐅Français.docx";
    let (source_content, _) = gen_bytes();

    op.write(source_path, source_content.clone()).await?;
    op.copy(source_path, target_path).await?;

    let target_content = op.read(target_path).await.expect("read must succeed");
    assert_eq!(target_content, source_content);

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
    let source_path = uuid::Uuid::new_v4().to_string();
    let (content, _) = gen_bytes();

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
    let (content, _) = gen_bytes();

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
    let (source_content, _) = gen_bytes();

    op.write(&source_path, source_content.clone()).await?;

    let target_path = format!(
        "{}/{}/{}",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4()
    );

    op.copy(&source_path, &target_path).await?;

    let target_content = op.read(&target_path).await.expect("read must succeed");
    assert_eq!(target_content, source_content);

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}

/// Copy to a exist path should overwrite successfully.
pub async fn test_copy_overwrite(op: Operator) -> Result<()> {
    let source_path = uuid::Uuid::new_v4().to_string();
    let (source_content, _) = gen_bytes();

    op.write(&source_path, source_content.clone()).await?;

    let target_path = uuid::Uuid::new_v4().to_string();
    let (target_content, _) = gen_bytes();
    assert_ne!(source_content, target_content);

    op.write(&target_path, target_content).await?;

    op.copy(&source_path, &target_path).await?;

    let target_content = op.read(&target_path).await.expect("read must succeed");
    assert_eq!(target_content, source_content);

    op.delete(&source_path).await.expect("delete must succeed");
    op.delete(&target_path).await.expect("delete must succeed");
    Ok(())
}
