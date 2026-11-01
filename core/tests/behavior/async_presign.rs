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
use http::header;
use log::debug;
use opendal::raw;
use reqwest::Url;

use crate::*;

pub fn tests(op: &Operator, tests: &mut Vec<Trial>) {
    let cap = op.info().capability();

    if cap.read && cap.write && cap.presign {
        tests.extend(async_trials!(
            op,
            test_presign_write,
            test_presign_read,
            test_presign_stat,
            test_presign_delete
        ))
    }
}

/// Presign write should succeed.
pub async fn test_presign_write(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", path);
    let (content, size) = gen_bytes(op.info().capability());

    let signed_req = op.presign_write(&path, Duration::from_secs(3600)).await?;
    debug!("Generated request: {signed_req:?}");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }
    req = req.header(header::CONTENT_LENGTH, content.len());
    req = req.body(reqwest::Body::from(content));

    let resp = req.send().await.expect("send request must succeed");
    debug!(
        "write response: {:?}",
        resp.text().await.expect("read response must succeed")
    );

    let meta = op.stat(&path).await.expect("stat must succeed");
    assert_eq!(meta.content_length(), size as u64);

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

pub async fn test_presign_stat(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", path);
    let (content, size) = gen_bytes(op.info().capability());
    op.write(&path, content.clone())
        .await
        .expect("write must succeed");
    let signed_req = op.presign_stat(&path, Duration::from_secs(3600)).await?;
    debug!("Generated request: {signed_req:?}");
    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }
    let resp = req.send().await.expect("send request must succeed");
    assert_eq!(resp.status(), http::StatusCode::OK, "status ok",);

    let content_length = raw::parse_content_length(resp.headers())
        .expect("no content length")
        .expect("content length must be present");
    assert_eq!(content_length, size as u64);

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

// Presign read should read content successfully.
pub async fn test_presign_read(op: Operator) -> Result<()> {
    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", path);
    let (content, size) = gen_bytes(op.info().capability());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let signed_req = op.presign_read(&path, Duration::from_secs(3600)).await?;
    debug!("Generated request: {signed_req:?}");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }

    let resp = req.send().await.expect("send request must succeed");

    let bs = resp.bytes().await.expect("read response must succeed");
    assert_eq!(size, bs.len(), "read size");
    assert_eq!(sha256_digest(&bs), sha256_digest(&content), "read content");

    op.delete(&path).await.expect("delete must succeed");
    Ok(())
}

/// Presign delete should succeed.
pub async fn test_presign_delete(op: Operator) -> Result<()> {
    let cap = op.info().capability();
    if !cap.presign_delete {
        return Ok(());
    }

    let path = uuid::Uuid::new_v4().to_string();
    debug!("Generate a random file: {}", path);
    let (content, _size) = gen_bytes(op.info().capability());
    // create a file
    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    // presign delete it
    let signed_req = op.presign_delete(&path, Duration::from_secs(3600)).await?;
    debug!("Generated request: {signed_req:?}");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }

    let _resp = req.send().await.expect("send request must succeed");

    assert!(!op.exists(&path).await.expect("delete must succeed"));
    Ok(())
}
