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

    if cap.read && cap.write && cap.compose {
        tests.extend(async_trials!(
            op,
            test_compose_ordered_sources,
            test_compose_incrementally,
            test_compose_empty,
            test_compose_self,
            test_compose_more_than_provider_request_limit
        ));
    }

    if cap.read && cap.write && cap.stat && cap.compose && cap.compose_with_content_type {
        tests.extend(async_trials!(op, test_compose_with_content_type));
    }

    if cap.read && cap.write && cap.stat && cap.compose && cap.compose_with_source_version {
        tests.extend(async_trials!(op, test_compose_with_source_version));
    }

    if cap.read
        && cap.write
        && cap.stat
        && cap.compose
        && (cap.compose_with_source_version || cap.compose_with_source_if_match)
    {
        tests.extend(async_trials!(op, test_compose_with_source_if_not_changed));
    }

    if cap.read && cap.write && cap.stat && cap.compose && cap.compose_with_if_version_match {
        tests.extend(async_trials!(op, test_compose_with_if_version_match));
    }

    if cap.read
        && cap.write
        && cap.stat
        && cap.compose
        && (cap.compose_with_if_version_match || cap.compose_with_if_match)
    {
        tests.extend(async_trials!(op, test_compose_with_if_not_changed));
    }

    if cap.read && cap.write && cap.compose && cap.compose_with_if_not_exists {
        tests.extend(async_trials!(op, test_compose_with_if_not_exists));
    }
}

pub async fn test_compose_ordered_sources(op: Operator) -> Result<()> {
    let first = TEST_FIXTURE.new_file_path();
    let second = TEST_FIXTURE.new_file_path();
    let target = TEST_FIXTURE.new_file_path();

    op.write(&first, "hello").await?;
    op.write(&second, " world").await?;
    op.write(&target, "stale").await?;

    op.compose([first.as_str(), second.as_str(), first.as_str()], &target)
        .await?;

    assert_eq!(
        op.read(&target).await?.to_bytes().as_ref(),
        b"hello worldhello"
    );
    assert_eq!(op.read(&first).await?.to_bytes().as_ref(), b"hello");
    assert_eq!(op.read(&second).await?.to_bytes().as_ref(), b" world");
    Ok(())
}

pub async fn test_compose_incrementally(op: Operator) -> Result<()> {
    let first = TEST_FIXTURE.new_file_path();
    let second = TEST_FIXTURE.new_file_path();
    let target = TEST_FIXTURE.new_file_path();

    op.write(&first, "a").await?;
    op.write(&second, "b").await?;

    let mut composer = op.composer_with(&target).concurrent(2).await?;
    composer.compose(first.as_str()).await?;
    composer
        .compose_options(second.as_str(), options::ComposeSourceOptions::default())
        .await?;
    let metadata = composer.close().await?;

    assert_eq!(metadata.content_length(), 2);
    assert_eq!(op.read(&target).await?.to_bytes().as_ref(), b"ab");
    assert_eq!(composer.close().await?.content_length(), 2);
    Ok(())
}

pub async fn test_compose_empty(op: Operator) -> Result<()> {
    let target = TEST_FIXTURE.new_file_path();
    let err = op
        .compose(Vec::<String>::new(), &target)
        .await
        .expect_err("empty composition must fail");
    assert_eq!(err.kind(), ErrorKind::ConfigInvalid);

    if op.info().capability().stat {
        assert!(!op.exists(&target).await?);
    }
    Ok(())
}

pub async fn test_compose_self(op: Operator) -> Result<()> {
    let path = TEST_FIXTURE.new_file_path();
    op.write(&path, "content").await?;

    let err = op
        .compose([path.as_str()], &path)
        .await
        .expect_err("composing a destination from itself must fail");
    assert_eq!(err.kind(), ErrorKind::IsSameFile);
    assert_eq!(op.read(&path).await?.to_bytes().as_ref(), b"content");
    Ok(())
}

pub async fn test_compose_more_than_provider_request_limit(op: Operator) -> Result<()> {
    let target = TEST_FIXTURE.new_file_path();
    let mut sources = Vec::with_capacity(65);

    for index in 0..65_u8 {
        let path = TEST_FIXTURE.new_file_path();
        op.write(&path, vec![index]).await?;
        sources.push(path);
    }

    op.compose_with(sources.iter().map(String::as_str), &target)
        .concurrent(4)
        .await?;

    assert_eq!(
        op.read(&target).await?.to_bytes(),
        (0..65_u8).collect::<Vec<_>>()
    );
    Ok(())
}

pub async fn test_compose_with_content_type(op: Operator) -> Result<()> {
    let source = TEST_FIXTURE.new_file_path();
    let target = TEST_FIXTURE.new_file_path();
    op.write(&source, "content").await?;

    op.compose_with([source.as_str()], &target)
        .content_type("text/plain")
        .await?;

    assert_eq!(op.stat(&target).await?.content_type(), Some("text/plain"));
    Ok(())
}

pub async fn test_compose_with_source_version(op: Operator) -> Result<()> {
    let source = TEST_FIXTURE.new_file_path();
    let target = TEST_FIXTURE.new_file_path();
    op.write(&source, "content").await?;
    let metadata = op.stat(&source).await?;
    let version = metadata
        .version()
        .expect("source version capability requires stat to return a version");

    let mut composer = op.composer(&target).await?;
    composer
        .compose_with(source.as_str())
        .version(version)
        .await?;
    composer.close().await?;

    assert_eq!(op.read(&target).await?.to_bytes().as_ref(), b"content");
    Ok(())
}

pub async fn test_compose_with_source_if_not_changed(op: Operator) -> Result<()> {
    let source = TEST_FIXTURE.new_file_path();
    let other = TEST_FIXTURE.new_file_path();
    let target = TEST_FIXTURE.new_file_path();
    let rejected_target = TEST_FIXTURE.new_file_path();

    op.write(&source, "source").await?;
    let expected = op.stat(&source).await?;

    op.write(&other, "other").await?;
    op.write(&other, "unrelated").await?;
    let unrelated = op.stat(&other).await?;

    op.compose(
        [(
            source.as_str(),
            options::ComposeSourceOptions {
                if_not_changed: Some(expected),
                ..Default::default()
            },
        )],
        &target,
    )
    .await?;

    assert_eq!(op.read(&target).await?.to_bytes().as_ref(), b"source");
    assert_eq!(op.read(&source).await?.to_bytes().as_ref(), b"source");

    op.compose(
        [(
            source.as_str(),
            options::ComposeSourceOptions {
                if_not_changed: Some(unrelated),
                ..Default::default()
            },
        )],
        &rejected_target,
    )
    .await
    .expect_err("an unrelated source identity must not be ignored");

    assert!(!op.exists(&rejected_target).await?);
    assert_eq!(op.read(&source).await?.to_bytes().as_ref(), b"source");
    Ok(())
}

pub async fn test_compose_with_if_version_match(op: Operator) -> Result<()> {
    let source = TEST_FIXTURE.new_file_path();
    let target = TEST_FIXTURE.new_file_path();
    op.write(&source, "new").await?;
    op.write(&target, "old").await?;
    let metadata = op.stat(&target).await?;
    let version = metadata
        .version()
        .expect("version match capability requires stat to return a version");

    op.compose_with([source.as_str()], &target)
        .if_version_match(version)
        .await?;

    assert_eq!(op.read(&target).await?.to_bytes().as_ref(), b"new");
    Ok(())
}

pub async fn test_compose_with_if_not_changed(op: Operator) -> Result<()> {
    let source = TEST_FIXTURE.new_file_path();
    let target = TEST_FIXTURE.new_file_path();

    op.write(&source, "composed").await?;
    op.write(&target, "initial").await?;
    let expected = op.stat(&target).await?;

    op.compose_with([source.as_str()], &target)
        .if_not_changed(&expected)
        .await?;
    assert_eq!(op.read(&target).await?.to_bytes().as_ref(), b"composed");

    let stale = op.stat(&target).await?;
    op.write(&target, "replacement").await?;
    let err = op
        .compose_with([source.as_str()], &target)
        .if_not_changed(&stale)
        .await
        .expect_err("stale destination metadata must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    assert_eq!(op.read(&target).await?.to_bytes().as_ref(), b"replacement");
    Ok(())
}

pub async fn test_compose_with_if_not_exists(op: Operator) -> Result<()> {
    let source = TEST_FIXTURE.new_file_path();
    let target = TEST_FIXTURE.new_file_path();
    op.write(&source, "new").await?;
    op.write(&target, "existing").await?;

    let err = op
        .compose_with([source.as_str()], &target)
        .if_not_exists(true)
        .await
        .expect_err("composition must not replace an existing destination");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    assert_eq!(op.read(&target).await?.to_bytes().as_ref(), b"existing");
    Ok(())
}
