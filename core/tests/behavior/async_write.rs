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

use anyhow::Result;
use bytes::Bytes;
use futures::AsyncWriteExt;
use futures::SinkExt;
use futures::StreamExt;
use futures::io::BufReader;
use futures::io::Cursor;
use futures::stream;

use crate::*;

pub fn tests(op: &Operator, tests: &mut Vec<Trial>) {
    let cap = op.info().capability();

    if cap.read && cap.write && cap.stat {
        tests.extend(async_trials!(
            op,
            test_write_only,
            test_write_with_empty_content,
            test_write_with_dir_path,
            test_write_with_special_chars,
            test_write_with_cache_control,
            test_write_with_content_type,
            test_write_with_content_disposition,
            test_write_with_content_encoding,
            test_write_with_if_none_match,
            test_write_with_if_not_exists,
            test_write_with_if_match,
            test_write_with_version_conditions,
            test_write_if_not_changed,
            test_write_with_user_metadata,
            test_write_returns_metadata,
            test_writer_write,
            test_writer_write_with_overwrite,
            test_writer_write_with_concurrent,
            test_writer_sink,
            test_writer_sink_with_concurrent,
            test_writer_abort,
            test_writer_abort_with_concurrent,
            test_writer_futures_copy,
            test_writer_futures_copy_with_concurrent,
            test_writer_return_metadata,
            test_writer_copy_from_interleaved,
            test_writer_write_non_contiguous_data,
            test_writer_write_with_if_not_exists,
            test_writer_write_with_if_none_match,
            test_writer_write_with_if_match,
            test_writer_write_with_version_conditions
        ))
    }

    if cap.read && cap.write && cap.write_can_append && cap.stat {
        tests.extend(async_trials!(
            op,
            test_write_with_append,
            test_write_with_append_returns_metadata,
            test_writer_with_append
        ))
    }
}

/// Write a single file and test with stat.
pub async fn test_write_only(op: Operator) -> Result<()> {
    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content).await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), size as u64);

    Ok(())
}

/// Write a file with empty content.
pub async fn test_write_with_empty_content(op: Operator) -> Result<()> {
    if !op.info().capability().write_can_empty {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();

    let bs: Vec<u8> = vec![];
    op.write(&path, bs).await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), 0);
    Ok(())
}

/// Write file with dir path should return an error
pub async fn test_write_with_dir_path(op: Operator) -> Result<()> {
    let path = TEST_FIXTURE.new_dir_path();

    let result = op.write(&path, vec![1]).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::IsADirectory);

    Ok(())
}

/// Write a single file with special chars should succeed.
pub async fn test_write_with_special_chars(op: Operator) -> Result<()> {
    // Ignore test for vercel blob https://github.com/apache/opendal/pull/4103.
    #[cfg(feature = "services-vercel-blob")]
    if op.info().scheme() == services::VERCEL_BLOB_SCHEME {
        log::warn!("ignore test for vercel blob https://github.com/apache/opendal/pull/4103");
        return Ok(());
    }

    let path = format!("nested/{} !@#$%^&()_+-=;',.txt", uuid::Uuid::new_v4());
    let (path, content, size) = TEST_FIXTURE.new_file_with_path(op.clone(), &path);

    op.write(&path, content).await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), size as u64);

    Ok(())
}

/// Write a single file with cache control should succeed.
pub async fn test_write_with_cache_control(op: Operator) -> Result<()> {
    if !op.info().capability().write_with_cache_control {
        return Ok(());
    }

    let path = uuid::Uuid::new_v4().to_string();
    let (content, _) = gen_bytes(op.info().capability());

    let target_cache_control = "no-cache, no-store, max-age=300";
    op.write_with(&path, content)
        .cache_control(target_cache_control)
        .await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(
        meta.cache_control().expect("cache control must exist"),
        target_cache_control
    );

    op.delete(&path).await.expect("delete must succeed");

    Ok(())
}

/// Write a single file with content type should succeed.
pub async fn test_write_with_content_type(op: Operator) -> Result<()> {
    if !op.info().capability().write_with_content_type {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());

    let target_content_type = "application/json";
    op.write_with(&path, content)
        .content_type(target_content_type)
        .await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(
        meta.content_type().expect("content type must exist"),
        target_content_type
    );
    assert_eq!(meta.content_length(), size as u64);

    Ok(())
}

/// Write a single file with content disposition should succeed.
pub async fn test_write_with_content_disposition(op: Operator) -> Result<()> {
    if !op.info().capability().write_with_content_disposition {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());

    let target_content_disposition = "attachment; filename=\"filename.jpg\"";
    op.write_with(&path, content)
        .content_disposition(target_content_disposition)
        .await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.mode(), EntryMode::FILE);
    assert_eq!(
        meta.content_disposition().expect("content type must exist"),
        target_content_disposition
    );
    assert_eq!(meta.content_length(), size as u64);

    Ok(())
}

/// Write a single file with content encoding should succeed.
pub async fn test_write_with_content_encoding(op: Operator) -> Result<()> {
    if !op.info().capability().write_with_content_encoding {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    let target_content_encoding = "gzip";
    op.write_with(&path, content)
        .content_encoding(target_content_encoding)
        .await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(
        meta.content_encoding()
            .expect("content encoding must exist"),
        target_content_encoding
    );
    Ok(())
}

/// write a single file with user defined metadata should succeed.
pub async fn test_write_with_user_metadata(op: Operator) -> Result<()> {
    if !op.info().capability().write_with_user_metadata {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());
    let target_user_metadata = vec![("location".to_string(), "everywhere".to_string())];
    op.write_with(&path, content)
        .user_metadata(target_user_metadata.clone())
        .await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    let resp_meta = meta.user_metadata().expect("meta data must exist");

    assert_eq!(
        resp_meta
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<HashMap<_, _>>(),
        target_user_metadata.into_iter().collect::<HashMap<_, _>>()
    );

    Ok(())
}

pub async fn test_write_returns_metadata(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    let meta = op.write(&path, content).await?;
    let stat_meta = op.stat(&path).await?;

    assert_metadata(stat_meta, meta);

    Ok(())
}

/// Delete existing file should succeed.
pub async fn test_writer_abort(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    let mut writer = match op.writer(&path).await {
        Ok(writer) => writer,
        Err(e) => {
            assert_eq!(e.kind(), ErrorKind::Unsupported);
            return Ok(());
        }
    };

    if let Err(e) = writer.write(content).await {
        assert_eq!(e.kind(), ErrorKind::Unsupported);
        return Ok(());
    }

    if let Err(e) = writer.abort().await {
        assert_eq!(e.kind(), ErrorKind::Unsupported);
        return Ok(());
    }

    // Aborted writer should not write actual file.
    assert!(!op.exists(&path).await?);
    Ok(())
}

/// Delete existing file should succeed.
pub async fn test_writer_abort_with_concurrent(op: Operator) -> Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    let mut writer = match op.writer_with(&path).concurrent(2).await {
        Ok(writer) => writer,
        Err(e) => {
            assert_eq!(e.kind(), ErrorKind::Unsupported);
            return Ok(());
        }
    };

    if let Err(e) = writer.write(content).await {
        assert_eq!(e.kind(), ErrorKind::Unsupported);
        return Ok(());
    }

    if let Err(e) = writer.abort().await {
        assert_eq!(e.kind(), ErrorKind::Unsupported);
        return Ok(());
    }

    // Aborted writer should not write actual file.
    assert!(!op.exists(&path).await?);
    Ok(())
}

/// Append data into writer
pub async fn test_writer_write(op: Operator) -> Result<()> {
    if !(op.info().capability().write_can_multi) {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let size = 5 * 1024 * 1024; // write file with 5 MiB
    let content_a = gen_fixed_bytes(size);
    let content_b = gen_fixed_bytes(size);

    let mut w = op.writer(&path).await?;
    w.write(content_a.clone()).await?;
    w.write(content_b.clone()).await?;
    w.close().await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), (size * 2) as u64);

    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(bs.len(), size * 2, "read size");
    assert_eq!(
        sha256_digest(&bs[..size]),
        sha256_digest(content_a),
        "read content a"
    );
    assert_eq!(
        sha256_digest(&bs[size..]),
        sha256_digest(content_b),
        "read content b"
    );

    Ok(())
}

/// Assemble a destination from local bytes and source ranges in call order.
pub async fn test_writer_copy_from_interleaved(op: Operator) -> Result<()> {
    if !op.info().capability().write_can_multi {
        return Ok(());
    }

    let source_path = TEST_FIXTURE.new_file_path();
    let target_path = TEST_FIXTURE.new_file_path();
    let source = gen_fixed_bytes(18 * 1024 * 1024);
    op.write(&source_path, source.clone()).await?;
    let source_meta = op.stat(&source_path).await?;
    let source_if_match =
        if op.info().capability().read_with_if_match && op.info().capability().stat_with_if_match {
            source_meta.etag().map(str::to_string)
        } else {
            None
        };

    let mut writer = op.writer(&target_path).await?;
    writer.write("header").await?;
    writer.copy_from(&source_path, 1024_u64..2048).await?;
    writer
        .copy_from_options(
            &source_path,
            options::ReadOptions {
                range: (2048_u64..14 * 1024 * 1024).into(),
                if_match: source_if_match.clone(),
                ..Default::default()
            },
        )
        .await?;
    writer.write("footer").await?;
    writer
        .copy_from_options(
            &source_path,
            options::ReadOptions {
                range: (15 * 1024 * 1024_u64..).into(),
                if_match: source_if_match,
                ..Default::default()
            },
        )
        .await?;
    writer.close().await?;

    let mut expected = Vec::new();
    expected.extend_from_slice(b"header");
    expected.extend_from_slice(&source[1024..14 * 1024 * 1024]);
    expected.extend_from_slice(b"footer");
    expected.extend_from_slice(&source[15 * 1024 * 1024..]);
    assert_eq!(op.read(&target_path).await?.to_bytes(), expected);
    Ok(())
}

/// Append data into writer
pub async fn test_writer_write_with_concurrent(op: Operator) -> Result<()> {
    if !(op.info().capability().write_can_multi) {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    // We need at least 3 part to make sure concurrent happened.
    let (content_a, size_a) = gen_bytes_with_range(5 * 1024 * 1024..6 * 1024 * 1024);
    let (content_b, size_b) = gen_bytes_with_range(5 * 1024 * 1024..6 * 1024 * 1024);
    let (content_c, size_c) = gen_bytes_with_range(5 * 1024 * 1024..6 * 1024 * 1024);

    let mut w = op.writer_with(&path).concurrent(3).await?;
    w.write(content_a.clone()).await?;
    w.write(content_b.clone()).await?;
    w.write(content_c.clone()).await?;
    w.close().await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), (size_a + size_b + size_c) as u64);

    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(bs.len(), size_a + size_b + size_c, "read size");
    assert_eq!(
        sha256_digest(&bs[..size_a]),
        sha256_digest(content_a),
        "read content a"
    );
    assert_eq!(
        sha256_digest(&bs[size_a..size_a + size_b]),
        sha256_digest(content_b),
        "read content b"
    );
    assert_eq!(
        sha256_digest(&bs[size_a + size_b..size_a + size_b + size_c]),
        sha256_digest(content_c),
        "read content b"
    );

    Ok(())
}

/// Streaming data into writer
pub async fn test_writer_sink(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !(cap.write && cap.write_can_multi) {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let size = 5 * 1024 * 1024; // write file with 5 MiB
    let content_a = gen_fixed_bytes(size);
    let content_b = gen_fixed_bytes(size);
    let mut stream = stream::iter(vec![
        Bytes::from(content_a.clone()),
        Bytes::from(content_b.clone()),
    ])
    .map(Ok);

    let mut w = op
        .writer_with(&path)
        .chunk(4 * 1024 * 1024)
        .await?
        .into_bytes_sink();
    w.send_all(&mut stream).await?;
    w.close().await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), (size * 2) as u64);

    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(bs.len(), size * 2, "read size");
    assert_eq!(
        sha256_digest(&bs[..size]),
        sha256_digest(content_a),
        "read content a"
    );
    assert_eq!(
        sha256_digest(&bs[size..]),
        sha256_digest(content_b),
        "read content b"
    );

    Ok(())
}

/// Streaming data into writer
pub async fn test_writer_sink_with_concurrent(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !(cap.write && cap.write_can_multi) {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let size = 8 * 1024 * 1024; // write file with 8 MiB
    let content_a = gen_fixed_bytes(size);
    let content_b = gen_fixed_bytes(size);
    let mut stream = stream::iter(vec![
        Bytes::from(content_a.clone()),
        Bytes::from(content_b.clone()),
    ])
    .map(Ok);

    let mut w = op
        .writer_with(&path)
        .chunk(5 * 1024 * 1024)
        .concurrent(4)
        .await?
        .into_bytes_sink();
    w.send_all(&mut stream).await?;
    w.close().await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), (size * 2) as u64);

    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(bs.len(), size * 2, "read size");
    assert_eq!(
        sha256_digest(&bs[..size]),
        sha256_digest(content_a),
        "read content a"
    );
    assert_eq!(
        sha256_digest(&bs[size..]),
        sha256_digest(content_b),
        "read content b"
    );

    Ok(())
}

/// Copy data from reader to writer
pub async fn test_writer_futures_copy(op: Operator) -> Result<()> {
    if !(op.info().capability().write_can_multi) {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let (content, size): (Vec<u8>, usize) =
        gen_bytes_with_range(10 * 1024 * 1024..20 * 1024 * 1024);

    let mut w = op
        .writer_with(&path)
        .chunk(8 * 1024 * 1024)
        .await?
        .into_futures_async_write();

    // Wrap a buf reader here to make sure content is read in 1MiB chunks.
    let mut cursor = BufReader::with_capacity(1024 * 1024, Cursor::new(content.clone()));
    futures::io::copy_buf(&mut cursor, &mut w).await?;
    w.close().await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), size as u64);

    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(bs.len(), size, "read size");
    assert_eq!(
        sha256_digest(&bs[..size]),
        sha256_digest(content),
        "read content"
    );

    Ok(())
}

/// Copy data from reader to writer
pub async fn test_writer_futures_copy_with_concurrent(op: Operator) -> Result<()> {
    if !(op.info().capability().write_can_multi) {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let (content, size): (Vec<u8>, usize) =
        gen_bytes_with_range(10 * 1024 * 1024..20 * 1024 * 1024);

    let mut w = op
        .writer_with(&path)
        .chunk(8 * 1024 * 1024)
        .concurrent(4)
        .await?
        .into_futures_async_write();

    // Wrap a buf reader here to make sure content is read in 1MiB chunks.
    let mut cursor = BufReader::with_capacity(1024 * 1024, Cursor::new(content.clone()));
    futures::io::copy_buf(&mut cursor, &mut w).await?;
    w.close().await.expect("close must succeed");

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), size as u64);

    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(bs.len(), size, "read size");
    assert_eq!(
        sha256_digest(&bs[..size]),
        sha256_digest(content),
        "read content"
    );

    Ok(())
}

pub async fn test_writer_return_metadata(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.write_can_multi {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let size = 5 * 1024 * 1024; // write file with 5 MiB
    let content_a = gen_fixed_bytes(size);
    let content_b = gen_fixed_bytes(size);

    let mut w = op.writer(&path).await?;
    w.write(content_a.clone()).await?;
    w.write(content_b.clone()).await?;
    let meta = w.close().await?;

    let stat_meta = op.stat(&path).await.expect("stat must succeed");

    assert_metadata(stat_meta, meta);

    Ok(())
}

/// Test append to a file must success.
pub async fn test_write_with_append(op: Operator) -> Result<()> {
    let path = TEST_FIXTURE.new_file_path();
    let (content_one, size_one) = gen_bytes(op.info().capability());
    let (content_two, size_two) = gen_bytes(op.info().capability());

    op.write_with(&path, content_one.clone())
        .append(true)
        .await
        .expect("append file first time must success");

    let meta = op.stat(&path).await?;
    assert_eq!(meta.content_length(), size_one as u64);

    op.write_with(&path, content_two.clone())
        .append(true)
        .await
        .expect("append to an existing file must success");

    let bs = op
        .read(&path)
        .await
        .expect("read file must success")
        .to_bytes();

    assert_eq!(bs.len(), size_one + size_two);
    assert_eq!(bs[..size_one], content_one);
    assert_eq!(bs[size_one..], content_two);

    Ok(())
}

pub async fn test_write_with_append_returns_metadata(op: Operator) -> Result<()> {
    let cap = op.info().capability();

    let path = TEST_FIXTURE.new_file_path();
    let (content_one, _) = gen_bytes(cap);
    let (content_two, _) = gen_bytes(cap);

    op.write_with(&path, content_one.clone())
        .append(true)
        .await
        .expect("append file first time must success");

    let meta = op
        .write_with(&path, content_two.clone())
        .append(true)
        .await
        .expect("append to an existing file must success");

    let stat_meta = op.stat(&path).await.expect("stat must succeed");
    assert_metadata(stat_meta, meta);

    Ok(())
}

fn assert_metadata(stat_meta: Metadata, meta: Metadata) {
    assert_eq!(stat_meta.content_length(), meta.content_length());
    if meta.etag().is_some() {
        assert_eq!(stat_meta.etag(), meta.etag());
    }
    if meta.last_modified().is_some() {
        assert_eq!(stat_meta.last_modified(), meta.last_modified());
    }
    if meta.version().is_some() {
        assert_eq!(stat_meta.version(), meta.version());
    }
    if meta.content_md5().is_some() {
        assert_eq!(stat_meta.content_md5(), meta.content_md5());
    }
    if meta.content_type().is_some() {
        assert_eq!(stat_meta.content_type(), meta.content_type());
    }
    if meta.content_encoding().is_some() {
        assert_eq!(stat_meta.content_encoding(), meta.content_encoding());
    }
    if meta.content_disposition().is_some() {
        assert_eq!(stat_meta.content_disposition(), meta.content_disposition());
    }
}

/// Copy data from reader to writer
pub async fn test_writer_with_append(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    let (content, size): (Vec<u8>, usize) =
        gen_bytes_with_range(10 * 1024 * 1024..20 * 1024 * 1024);

    let mut a = op
        .writer_with(&path)
        .append(true)
        .await?
        .into_futures_async_write();

    // Wrap a buf reader here to make sure content is read in 1MiB chunks.
    let mut cursor = BufReader::with_capacity(1024 * 1024, Cursor::new(content.clone()));
    futures::io::copy_buf(&mut cursor, &mut a).await?;
    a.close().await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), size as u64);

    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(bs.len(), size, "read size");
    assert_eq!(
        sha256_digest(&bs[..size]),
        sha256_digest(content),
        "read content"
    );

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

pub async fn test_writer_write_with_overwrite(op: Operator) -> Result<()> {
    // ghac does not support overwrite
    #[cfg(feature = "services-ghac")]
    if op.info().scheme() == services::GHAC_SCHEME {
        return Ok(());
    }

    let path = uuid::Uuid::new_v4().to_string();
    let (content_one, _) = gen_bytes(op.info().capability());
    let (content_two, _) = gen_bytes(op.info().capability());

    op.write(&path, content_one.clone()).await?;
    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(
        sha256_digest(&bs),
        sha256_digest(&content_one),
        "read content_one"
    );
    op.write(&path, content_two.clone())
        .await
        .expect("write overwrite must succeed");
    let bs = op.read(&path).await?.to_bytes();
    assert_ne!(
        sha256_digest(&bs),
        sha256_digest(&content_one),
        "content_one must be overwrote"
    );
    assert_eq!(
        sha256_digest(&bs),
        sha256_digest(&content_two),
        "read content_two"
    );

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

/// Write an exists file with if_none_match should match, else get a ConditionNotMatch error.
pub async fn test_write_with_if_none_match(op: Operator) -> Result<()> {
    if !op.info().capability().write_with_if_none_match {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&path).await?;

    let res = op
        .write_with(&path, content.clone())
        .if_none_match(meta.etag().expect("etag must exist"))
        .await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Write a file with if_not_exists will get a ConditionNotMatch error if file exists.
pub async fn test_write_with_if_not_exists(op: Operator) -> Result<()> {
    if !op.info().capability().write_with_if_not_exists {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    let res = op
        .write_with(&path, content.clone())
        .if_not_exists(true)
        .await;
    assert!(res.is_ok());

    let res = op
        .write_with(&path, content.clone())
        .if_not_exists(true)
        .await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Write a file with if_match will get a ConditionNotMatch error if file's etag does not match.
pub async fn test_write_with_if_match(op: Operator) -> Result<()> {
    if !op.info().capability().write_with_if_match {
        return Ok(());
    }

    // Create two different files with different content
    let (path_a, content_a, _) = TEST_FIXTURE.new_file(op.clone());
    let (path_b, content_b, _) = TEST_FIXTURE.new_file(op.clone());

    // Write initial content to both files
    op.write(&path_a, content_a.clone()).await?;
    op.write(&path_b, content_b.clone()).await?;

    // Get etags for both files
    let meta_a = op.stat(&path_a).await?;
    let etag_a = meta_a.etag().expect("etag must exist");
    let meta_b = op.stat(&path_b).await?;
    let etag_b = meta_b.etag().expect("etag must exist");

    // Should succeed: Writing to path_a with its own etag
    let res = op
        .write_with(&path_a, content_a.clone())
        .if_match(etag_a)
        .await;
    assert!(res.is_ok());

    // Should fail: Writing to path_a with path_b's etag
    let res = op
        .write_with(&path_a, content_a.clone())
        .if_match(etag_b)
        .await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Version preconditions should compare against the current live object version.
pub async fn test_write_with_version_conditions(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.write_with_if_version_match || !cap.write_with_if_version_not_match {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let (initial, _) = gen_bytes(cap);
    let (replacement, _) = gen_bytes(cap);
    assert_ne!(initial, replacement);

    op.write(&path, initial).await?;
    let first_version = op
        .stat(&path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();

    op.write_with(&path, replacement.clone())
        .if_version_match(&first_version)
        .await?;

    let current_version = op
        .stat(&path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();
    let err = op
        .write_with(&path, replacement.clone())
        .if_version_match(&first_version)
        .await
        .expect_err("stale version match must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    let err = op
        .write_with(&path, replacement.clone())
        .if_version_not_match(&current_version)
        .await
        .expect_err("equal version non-match must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    op.write_with(&path, replacement)
        .if_version_not_match(&first_version)
        .await?;

    let missing = TEST_FIXTURE.new_file_path();
    for result in [
        op.write_with(&missing, Vec::<u8>::new())
            .if_version_match(&current_version)
            .await,
        op.write_with(&missing, Vec::<u8>::new())
            .if_version_not_match(&current_version)
            .await,
    ] {
        assert_eq!(
            result.expect_err("missing target must fail").kind(),
            ErrorKind::ConditionNotMatch
        );
    }

    Ok(())
}

/// `if_not_changed` should accept the observed state once and reject it after replacement.
pub async fn test_write_if_not_changed(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.write_with_if_version_match && !cap.write_with_if_match {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let (initial, _) = gen_bytes(cap);
    let (replacement, _) = gen_bytes(cap);
    assert_ne!(initial, replacement);

    op.write(&path, initial).await?;
    let expected = op.stat(&path).await?;

    let mut conflicting = options::WriteOptions {
        if_not_changed: Some(expected.clone()),
        ..Default::default()
    };
    let mut matching = conflicting.clone();
    if cap.write_with_if_version_match {
        let version = expected.version().expect("version must exist");
        conflicting.if_version_match = Some(format!("different-{version}"));
        matching.if_version_match = Some(version.to_string());
    } else {
        let etag = expected.etag().expect("etag must exist");
        conflicting.if_match = Some(format!("different-{etag}"));
        matching.if_match = Some(etag.to_string());
    }

    let err = op
        .write_options(&path, replacement.clone(), conflicting)
        .await
        .expect_err("conflicting explicit condition must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    op.write_options(&path, replacement.clone(), matching)
        .await?;
    let err = op
        .write_with(&path, replacement)
        .if_not_changed(&expected)
        .await
        .expect_err("stale metadata must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Write an existing file through a chunked writer with if_not_exists should get a
/// ConditionNotMatch error.
pub async fn test_writer_write_with_if_not_exists(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.write_with_if_not_exists || !cap.write_can_multi {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let content = gen_fixed_bytes(cap.write_multi_min_size.unwrap_or(1));

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    // Some services reject the precondition when the writer is created or on an early
    // write rather than at commit time
    let res: opendal::Result<()> = async {
        let mut w = op.writer_with(&path).if_not_exists(true).await?;
        w.write(content.clone()).await?;
        w.write(content.clone()).await?;
        w.close().await?;
        Ok(())
    }
    .await;
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Write an existing file through a chunked writer with its own etag as if_none_match
/// should get a ConditionNotMatch error.
pub async fn test_writer_write_with_if_none_match(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.write_with_if_none_match || !cap.write_can_multi {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let content = gen_fixed_bytes(cap.write_multi_min_size.unwrap_or(1));

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&path).await?;
    let etag = meta.etag().expect("etag must exist");

    let res: opendal::Result<()> = async {
        let mut w = op.writer_with(&path).if_none_match(etag).await?;
        w.write(content.clone()).await?;
        w.write(content.clone()).await?;
        w.close().await?;
        Ok(())
    }
    .await;
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Write a file through a chunked writer with if_match should succeed with the file's own
/// etag and get a ConditionNotMatch error with a stale one.
pub async fn test_writer_write_with_if_match(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.write_with_if_match || !cap.write_can_multi {
        return Ok(());
    }

    let path_a = TEST_FIXTURE.new_file_path();
    let content_a = gen_fixed_bytes(cap.write_multi_min_size.unwrap_or(1));
    let (path_b, content_b, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path_a, content_a.clone()).await?;
    op.write(&path_b, content_b.clone()).await?;

    let etag_a = op
        .stat(&path_a)
        .await?
        .etag()
        .expect("etag must exist")
        .to_string();
    let etag_b = op
        .stat(&path_b)
        .await?
        .etag()
        .expect("etag must exist")
        .to_string();

    let mut w = op.writer_with(&path_a).if_match(&etag_a).await?;
    w.write(content_a.clone()).await?;
    w.write(content_a.clone()).await?;
    w.close().await.expect("close with own etag must succeed");

    // Should fail: writing to path_a with path_b's etag.
    let res: opendal::Result<()> = async {
        let mut w = op.writer_with(&path_a).if_match(&etag_b).await?;
        w.write(content_a.clone()).await?;
        w.write(content_a.clone()).await?;
        w.close().await?;
        Ok(())
    }
    .await;
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

/// Chunked writers should preserve version preconditions through final commit.
pub async fn test_writer_write_with_version_conditions(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.write_can_multi
        || !cap.write_with_if_version_match
        || !cap.write_with_if_version_not_match
    {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_file_path();
    let body = gen_fixed_bytes(cap.write_multi_min_size.unwrap_or(1));
    op.write(&path, body.clone()).await?;
    let expected = op
        .stat(&path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();

    let mut writer = op.writer_with(&path).if_version_match(&expected).await?;
    writer.write(body.clone()).await?;
    writer.write(body.clone()).await?;
    writer.close().await?;

    let current = op
        .stat(&path)
        .await?
        .version()
        .expect("version must exist")
        .to_string();
    let result: opendal::Result<()> = async {
        let mut writer = op.writer_with(&path).if_version_not_match(&current).await?;
        writer.write(body.clone()).await?;
        writer.write(body).await?;
        writer.close().await?;
        Ok(())
    }
    .await;
    assert_eq!(result.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    Ok(())
}

pub async fn test_writer_write_non_contiguous_data(op: Operator) -> Result<()> {
    let path = TEST_FIXTURE.new_file_path();
    let size = 1024 * 1024; // write file with 1 MiB
    let content_a = gen_fixed_bytes(size);
    let digest_a = sha256_digest(&content_a);
    let content_b = gen_fixed_bytes(size);
    let digest_b = sha256_digest(&content_b);

    let mut w = op.writer(&path).await?;
    w.write(vec![Bytes::from(content_a), Bytes::from(content_b)])
        .await?;
    w.close().await?;

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), (size * 2) as u64);

    let bs = op.read(&path).await?.to_bytes();
    assert_eq!(bs.len(), size * 2, "read size");
    assert_eq!(sha256_digest(&bs[..size]), digest_a, "read content a");
    assert_eq!(sha256_digest(&bs[size..]), digest_b, "read content b");

    Ok(())
}
