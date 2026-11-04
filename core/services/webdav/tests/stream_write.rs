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

//! Streaming write tests for the WebDAV service.
//!
//! A streaming writer performs multiple writes on one object. A wasm client
//! relies on this to upload files larger than one internal buffer without
//! keeping the whole file in memory. These tests drive the service through a
//! mock HTTP transport, so they need no WebDAV server.

use {
    futures::{AsyncWriteExt, stream},
    http::{Method, Request, Response, StatusCode, header},
    opendal_core::{
        Buffer, HttpBody, HttpTransport, HttpTransporter, OperationContext, Operator, Result,
    },
    opendal_service_webdav::Webdav,
    std::sync::{Arc, Mutex},
};

/// An HTTP request recorded by [`MockTransport`].
#[derive(Clone, Debug)]
struct RecordedRequest {
    method: Method,
    content_range: Option<String>,
    body: Vec<u8>,
}

/// A mock transport that records requests and answers them like a WebDAV server.
#[derive(Clone, Debug, Default)]
struct MockTransport {
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl MockTransport {
    /// Returns all requests recorded so far.
    fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl HttpTransport for MockTransport {
    async fn fetch(&self, request: Request<Buffer>) -> Result<Response<HttpBody>> {
        let content_range = request
            .headers()
            .get(header::CONTENT_RANGE)
            .map(|value| value.to_str().unwrap().to_owned());
        let recorded = RecordedRequest {
            method: request.method().clone(),
            content_range,
            body: request.body().to_bytes().to_vec(),
        };
        self.requests.lock().unwrap().push(recorded.clone());

        // The service stats the parent directory with PROPFIND before MKCOL.
        let status = match recorded.method.as_str() {
            "PUT" if recorded.content_range.is_some() => StatusCode::NO_CONTENT,
            "PUT" => StatusCode::CREATED,
            "MKCOL" => StatusCode::CREATED,
            "PROPFIND" => StatusCode::NOT_FOUND,
            _ => StatusCode::OK,
        };
        let body = HttpBody::new(stream::empty::<Result<Buffer>>(), Some(0));
        Ok(Response::builder().status(status).body(body).unwrap())
    }
}

/// Builds an operator whose requests are handled by the mock transport.
fn operator(transport: MockTransport) -> Operator {
    let webdav = Webdav::default().endpoint("http://webdav.example.com");
    let context = OperationContext::new().with_http_transport(HttpTransporter::new(transport));
    Operator::new(webdav).unwrap().with_context(context)
}

/// The service reports that one writer accepts multiple writes, which the
/// streaming upload path relies on.
#[tokio::test]
async fn capability_reports_stream_write_support() {
    let operator = operator(MockTransport::default());

    assert!(operator.info().capability().write_can_multi);
}

/// Writing more than one internal buffer through a single writer uploads every
/// chunk in order and keeps the content intact.
#[tokio::test]
async fn stream_write_larger_than_buffer_uploads_all_chunks() {
    let transport = MockTransport::default();
    let operator = operator(transport.clone());

    let payload = (0..600 * 1024)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<u8>>();
    let mut writer = operator
        .writer("big.bin")
        .await
        .unwrap()
        .into_futures_async_write();
    writer.write_all(&payload).await.unwrap();
    writer.close().await.unwrap();

    // The first chunk is a regular PUT; the following chunks append with
    // Content-Range so the server keeps the earlier chunks.
    let puts = transport
        .requests()
        .into_iter()
        .filter(|request| request.method == Method::PUT)
        .collect::<Vec<_>>();
    assert_eq!(puts.len(), 3);
    assert_eq!(puts[0].content_range, None);
    assert_eq!(
        puts[1].content_range.as_deref(),
        Some("bytes 262144-524287/*")
    );
    assert_eq!(
        puts[2].content_range.as_deref(),
        Some("bytes 524288-614399/*")
    );
    let uploaded = puts
        .iter()
        .flat_map(|put| put.body.iter().copied())
        .collect::<Vec<u8>>();
    assert_eq!(uploaded, payload);
}
