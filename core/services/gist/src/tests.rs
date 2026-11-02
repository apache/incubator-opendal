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

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use opendal_core::raw::oio::List;
use opendal_core::raw::oio::ReadStream;
use opendal_core::raw::*;
use opendal_core::*;

use crate::Gist;

const GIST_ID: &str = "aa5a315d61ae9438b18d";
const RAW_URL: &str = "https://gist.githubusercontent.com/octocat/aa5a315d61ae9438b18d/raw/db9c55113504e46fa076e7df3a04ce592e2e86d8/hello.txt";

#[derive(Debug)]
struct Recorded {
    method: String,
    uri: String,
    range: Option<String>,
    authorization: bool,
    body: String,
}

struct FakeInner {
    requests: Mutex<Vec<Recorded>>,
    responses: Mutex<VecDeque<(StatusCode, String)>>,
}

/// Serves canned responses in order and records what the service sent.
#[derive(Clone)]
struct FakeTransport {
    inner: Arc<FakeInner>,
}

impl FakeTransport {
    fn new(responses: Vec<(StatusCode, String)>) -> Self {
        Self {
            inner: Arc::new(FakeInner {
                requests: Mutex::new(Vec::new()),
                responses: Mutex::new(responses.into()),
            }),
        }
    }

    fn requests(&self) -> Vec<Recorded> {
        let mut requests = self.inner.requests.lock().unwrap();
        std::mem::take(&mut *requests)
    }
}

impl HttpTransport for FakeTransport {
    async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
        let body = req.body().to_bytes();

        self.inner.requests.lock().unwrap().push(Recorded {
            method: req.method().to_string(),
            uri: req.uri().to_string(),
            range: req
                .headers()
                .get(header::RANGE)
                .map(|value| value.to_str().unwrap().to_string()),
            authorization: req.headers().contains_key(header::AUTHORIZATION),
            body: String::from_utf8_lossy(&body).into_owned(),
        });

        let (status, body) = self
            .inner
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("fake transport ran out of responses");

        let len = body.len() as u64;
        let stream = futures::stream::iter([Ok(Buffer::from(body))]);

        Ok(Response::builder()
            .status(status)
            .header(header::CONTENT_LENGTH, len)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(HttpBody::new(stream, Some(len)))
            .expect("response must build"))
    }
}

fn build(transport: &FakeTransport) -> (impl Service, OperationContext) {
    let service = Gist::default()
        .gist_id(GIST_ID)
        .token("test-token")
        .build()
        .expect("gist service must build");
    let ctx = OperationContext::new().with_http_transport(HttpTransporter::new(transport.clone()));

    (service, ctx)
}

fn gist_json(files: &[(&str, &str)]) -> String {
    let files = files
        .iter()
        .map(|(name, content)| {
            format!(
                r#""{name}":{{"filename":"{name}","type":"text/plain","size":{},"raw_url":"{RAW_URL}","truncated":false,"content":"{content}"}}"#,
                content.len()
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(r#"{{"files":{{{files}}}}}"#)
}

#[tokio::test]
async fn list_root_returns_file_entries() {
    let transport = FakeTransport::new(vec![(
        StatusCode::OK,
        gist_json(&[("b.txt", "bb"), ("a.txt", "a")]),
    )]);
    let (service, ctx) = build(&transport);

    let mut lister = service
        .list(&ctx, "/", OpList::default())
        .expect("lister must be created");

    let mut entries = Vec::new();
    while let Some(entry) = lister.next().await.expect("list must succeed") {
        entries.push((entry.path().to_string(), entry.metadata().content_length()));
    }

    assert_eq!(
        entries,
        vec![("a.txt".to_string(), 1), ("b.txt".to_string(), 2)]
    );

    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].uri,
        format!("https://api.github.com/gists/{GIST_ID}")
    );
}

#[tokio::test]
async fn list_non_root_path_is_empty() {
    let transport = FakeTransport::new(vec![]);
    let (service, ctx) = build(&transport);

    let mut lister = service
        .list(&ctx, "dir/", OpList::default())
        .expect("lister must be created");

    assert!(lister.next().await.expect("list must succeed").is_none());
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn stat_missing_file_is_not_found() {
    let transport = FakeTransport::new(vec![(StatusCode::OK, gist_json(&[("a.txt", "a")]))]);
    let (service, ctx) = build(&transport);

    let err = service
        .stat(&ctx, "missing.txt", OpStat::default())
        .await
        .expect_err("stat of a missing file must fail");

    assert_eq!(err.kind(), ErrorKind::NotFound);
}

#[tokio::test]
async fn stat_root_is_a_directory() {
    let transport = FakeTransport::new(vec![]);
    let (service, ctx) = build(&transport);

    let rp = service
        .stat(&ctx, "/", OpStat::default())
        .await
        .expect("stat of the root must succeed");

    assert!(rp.into_metadata().is_dir());
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn write_sends_one_gist_edit() {
    let transport = FakeTransport::new(vec![(StatusCode::OK, gist_json(&[("a.txt", "hello")]))]);
    let (service, ctx) = build(&transport);

    let mut writer = service
        .write(&ctx, "a.txt", OpWrite::default())
        .expect("writer must be created");
    oio::Write::write(&mut writer, Buffer::from("hello"))
        .await
        .expect("write must be buffered");
    let metadata = oio::Write::close(&mut writer)
        .await
        .expect("close must write the file");

    assert_eq!(metadata.content_length(), 5);

    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PATCH");
    assert_eq!(
        requests[0].body,
        r#"{"files":{"a.txt":{"content":"hello"}}}"#
    );
}

#[tokio::test]
async fn write_rejects_content_outside_utf8() {
    let transport = FakeTransport::new(vec![]);
    let (service, ctx) = build(&transport);

    let mut writer = service
        .write(&ctx, "a.bin", OpWrite::default())
        .expect("writer must be created");
    oio::Write::write(&mut writer, Buffer::from(vec![0xff, 0xfe]))
        .await
        .expect("write must be buffered");

    let err = oio::Write::close(&mut writer)
        .await
        .expect_err("non UTF-8 content cannot be stored in a gist");

    assert_eq!(err.kind(), ErrorKind::Unsupported);
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn nested_path_is_unsupported() {
    let transport = FakeTransport::new(vec![]);
    let (service, ctx) = build(&transport);

    let err = service
        .stat(&ctx, "dir/a.txt", OpStat::default())
        .await
        .expect_err("a gist has no directories");

    assert_eq!(err.kind(), ErrorKind::Unsupported);
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn delete_missing_file_is_a_noop() {
    let transport = FakeTransport::new(vec![(StatusCode::OK, gist_json(&[("a.txt", "a")]))]);
    let (service, ctx) = build(&transport);

    let mut deleter = service.delete(&ctx).expect("deleter must be created");
    oio::Delete::delete(&mut deleter, "missing.txt", OpDelete::default())
        .await
        .expect("deleting a missing file must succeed");

    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
}

#[tokio::test]
async fn delete_removes_the_file_from_the_gist() {
    let transport = FakeTransport::new(vec![
        (StatusCode::OK, gist_json(&[("a.txt", "a")])),
        (StatusCode::OK, gist_json(&[("b.txt", "b")])),
    ]);
    let (service, ctx) = build(&transport);

    let mut deleter = service.delete(&ctx).expect("deleter must be created");
    oio::Delete::delete(&mut deleter, "a.txt", OpDelete::default())
        .await
        .expect("delete must succeed");

    let requests = transport.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].method, "PATCH");
    assert_eq!(requests[1].body, r#"{"files":{"a.txt":null}}"#);
}

#[tokio::test]
async fn read_returns_inline_content() {
    let transport = FakeTransport::new(vec![(
        StatusCode::OK,
        gist_json(&[("hello.txt", "0123456789")]),
    )]);
    let (service, ctx) = build(&transport);

    let reader = service
        .read(&ctx, "hello.txt", OpRead::default())
        .expect("reader must be created");
    let (rp, mut stream) = oio::Read::open(&reader, BytesRange::default())
        .await
        .expect("open must succeed");

    assert_eq!(
        rp.metadata()
            .expect("read must return metadata")
            .content_length(),
        10
    );
    assert_eq!(
        stream.read_all().await.expect("read must succeed").to_vec(),
        b"0123456789"
    );
    assert_eq!(transport.requests().len(), 1);
}

#[tokio::test]
async fn read_slices_inline_content_for_ranges() {
    let transport = FakeTransport::new(vec![(
        StatusCode::OK,
        gist_json(&[("hello.txt", "0123456789")]),
    )]);
    let (service, ctx) = build(&transport);

    let reader = service
        .read(&ctx, "hello.txt", OpRead::default())
        .expect("reader must be created");
    let (rp, mut stream) = oio::Read::open(&reader, BytesRange::new(2, Some(3)))
        .await
        .expect("open must succeed");

    // Read metadata reports the whole file, not the requested range.
    let metadata = rp.metadata().expect("read must return metadata");
    assert_eq!(metadata.content_length(), 10);
    assert_eq!(
        stream.read_all().await.expect("read must succeed").to_vec(),
        b"234"
    );
}

#[tokio::test]
async fn read_truncated_file_falls_back_to_raw_url() {
    let truncated = format!(
        r#"{{"files":{{"hello.txt":{{"filename":"hello.txt","type":"text/plain","size":20,"raw_url":"{RAW_URL}","truncated":true,"content":"0123"}}}}}}"#
    );
    let transport = FakeTransport::new(vec![
        (StatusCode::OK, truncated),
        (StatusCode::PARTIAL_CONTENT, "456".to_string()),
    ]);
    let (service, ctx) = build(&transport);

    let reader = service
        .read(&ctx, "hello.txt", OpRead::default())
        .expect("reader must be created");
    let (rp, mut stream) = oio::Read::open(&reader, BytesRange::new(4, Some(3)))
        .await
        .expect("open must succeed");

    assert_eq!(
        rp.metadata()
            .expect("read must return metadata")
            .content_length(),
        20
    );
    assert_eq!(
        stream.read_all().await.expect("read must succeed").to_vec(),
        b"456"
    );

    let requests = transport.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].uri, RAW_URL);
    assert_eq!(requests[1].range.as_deref(), Some("bytes=4-6"));
    assert!(
        !requests[1].authorization,
        "the GitHub token must not be sent to the raw content host"
    );
}
