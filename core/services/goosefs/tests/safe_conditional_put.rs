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

//! Acceptance tests for GooseFS safe Create / conditional rename (doc §8.8).
//!
//! Requires a live GooseFS master. Skip unless `OPENDAL_GOOSEFS_MASTER_ADDR`
//! is set (same env the behavior CI fixture exports).

use std::sync::Arc;

use opendal::ErrorKind;
use opendal::Operator;
use opendal::services::GooseFs;

fn maybe_operator() -> Option<Operator> {
    let master = std::env::var("OPENDAL_GOOSEFS_MASTER_ADDR").ok()?;
    if master.trim().is_empty() {
        return None;
    }
    let root = std::env::var("OPENDAL_GOOSEFS_ROOT").unwrap_or_else(|_| "/".to_string());
    let write_type =
        std::env::var("OPENDAL_GOOSEFS_WRITE_TYPE").unwrap_or_else(|_| "must_cache".to_string());

    let builder = GooseFs::default()
        .root(&root)
        .master_addr(&master)
        .write_type(&write_type);
    Some(Operator::new(builder).expect("build GooseFS operator"))
}

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[tokio::test]
async fn capability_declares_rename_with_if_not_exists() {
    let Some(op) = maybe_operator() else {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR unset");
        return;
    };
    let cap = op.info().capability();
    assert!(cap.write_with_if_not_exists);
    assert!(cap.rename_with_if_not_exists);
}

/// T0 / T4: conditional rename against an existing dst → ConditionNotMatch;
/// destination content is unchanged (Master no-replace).
#[tokio::test]
async fn rename_if_not_exists_rejects_existing_dst() {
    let Some(op) = maybe_operator() else {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR unset");
        return;
    };

    let src = unique("t0-src");
    let dst = unique("t0-dst");
    op.write(&src, "from-src").await.expect("write src");
    op.write(&dst, "dst-original").await.expect("write dst");

    let err = op
        .rename_with(&src, &dst)
        .if_not_exists(true)
        .await
        .expect_err("rename must fail when dst exists");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    let dst_after = op.read(&dst).await.expect("read dst").to_bytes();
    assert_eq!(&dst_after[..], b"dst-original");

    let _ = op.delete(&src).await;
    let _ = op.delete(&dst).await;
}

/// T3: serial second Create (write_with if_not_exists) must fail.
#[tokio::test]
async fn write_if_not_exists_serial_conflict() {
    let Some(op) = maybe_operator() else {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR unset");
        return;
    };

    let path = unique("t3-create");
    op.write_with(&path, "winner")
        .if_not_exists(true)
        .await
        .expect("first Create succeeds");

    let err = op
        .write_with(&path, "loser")
        .if_not_exists(true)
        .await
        .expect_err("second Create must fail");
    assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);

    let body = op.read(&path).await.expect("read").to_bytes();
    assert_eq!(&body[..], b"winner");

    let meta = op.stat(&path).await.expect("stat");
    assert!(
        meta.etag().is_some(),
        "P1 etag (file_id) should be present after write"
    );

    let _ = op.delete(&path).await;
}

/// T5: overwrite (if_not_exists=false) still replaces an existing file.
#[tokio::test]
async fn overwrite_rename_still_works() {
    let Some(op) = maybe_operator() else {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR unset");
        return;
    };

    let src = unique("t5-src");
    let dst = unique("t5-dst");
    op.write(&src, "new-content").await.expect("write src");
    op.write(&dst, "old-content").await.expect("write dst");

    op.rename(&src, &dst).await.expect("overwrite rename");

    let body = op.read(&dst).await.expect("read dst").to_bytes();
    assert_eq!(&body[..], b"new-content");

    let _ = op.delete(&dst).await;
}

/// T1: concurrent Create on the same path — exactly one wins; content is winner's.
#[tokio::test]
async fn concurrent_write_if_not_exists_exactly_one_wins() {
    let Some(op) = maybe_operator() else {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR unset");
        return;
    };
    let op = Arc::new(op);
    let path = unique("t1-concurrent");

    let rounds = 8u32;
    for round in 0..rounds {
        let p = format!("{path}-r{round}");
        let op_a = op.clone();
        let op_b = op.clone();
        let path_a = p.clone();
        let path_b = p.clone();

        let (ra, rb) = tokio::join!(
            async move {
                op_a.write_with(&path_a, "A")
                    .if_not_exists(true)
                    .await
                    .map(|_| "A")
            },
            async move {
                op_b.write_with(&path_b, "B")
                    .if_not_exists(true)
                    .await
                    .map(|_| "B")
            },
        );

        match (&ra, &rb) {
            (Ok(w), Err(e)) => {
                assert_eq!(e.kind(), ErrorKind::ConditionNotMatch);
                let body = op.read(&p).await.expect("read").to_bytes();
                assert_eq!(&body[..], w.as_bytes());
            }
            (Err(e), Ok(w)) => {
                assert_eq!(e.kind(), ErrorKind::ConditionNotMatch);
                let body = op.read(&p).await.expect("read").to_bytes();
                assert_eq!(&body[..], w.as_bytes());
            }
            (Ok(_), Ok(_)) => panic!("both concurrent Creates succeeded for {p}"),
            (Err(ea), Err(eb)) => panic!("both concurrent Creates failed for {p}: {ea:?} / {eb:?}"),
        }

        let _ = op.delete(&p).await;
    }
}

/// Nested write: CreateFile(recursive) on the temp sibling creates the parent,
/// so publish rename must succeed without an eager CreateDirectory.
#[tokio::test]
async fn write_nested_path_creates_parents() {
    let Some(op) = maybe_operator() else {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR unset");
        return;
    };

    let path = format!("{}/nested/file", unique("nested-write"));
    op.write(&path, "nested-body")
        .await
        .expect("write into missing parents");
    let body = op.read(&path).await.expect("read").to_bytes();
    assert_eq!(&body[..], b"nested-body");
    let _ = op.delete(&path).await;
}

/// Public rename still creates a destination parent that does not exist yet.
#[tokio::test]
async fn rename_creates_missing_destination_parent() {
    let Some(op) = maybe_operator() else {
        eprintln!("skip: OPENDAL_GOOSEFS_MASTER_ADDR unset");
        return;
    };

    let src = unique("mkdir-src");
    op.write(&src, "payload").await.expect("write src");
    let dst = format!("{}/nested/file", unique("mkdir-dst"));
    op.rename(&src, &dst)
        .await
        .expect("rename should create missing parent");
    let body = op.read(&dst).await.expect("read dst").to_bytes();
    assert_eq!(&body[..], b"payload");
    let _ = op.delete(&dst).await;
}
