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

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use http::{Request, Response, StatusCode};
use opendal::layers::RetryLayer;
use opendal::{Buffer, Error, ErrorKind, HttpBody, HttpTransport, HttpTransporter};
use opendal::{OperationContext, Operator, Result, services};

#[derive(Clone, Copy)]
enum TokenFailure {
    Temporary,
    Permanent,
    Unauthorized,
}

struct TokenTransport {
    failure: TokenFailure,
    token_requests: Arc<AtomicUsize>,
    list_requests: Arc<AtomicUsize>,
}

impl HttpTransport for TokenTransport {
    async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
        if req.uri().host() == Some("oauth2.googleapis.com") {
            assert_eq!(req.method(), http::Method::POST);
            assert_eq!(req.uri().path(), "/token");
            let attempt = self.token_requests.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                match self.failure {
                    TokenFailure::Temporary => {
                        return Err(Error::new(
                            ErrorKind::Unexpected,
                            "controlled token connection reset",
                        )
                        .set_temporary());
                    }
                    TokenFailure::Permanent => {
                        return Err(Error::new(
                            ErrorKind::Unexpected,
                            "controlled permanent token failure",
                        ));
                    }
                    TokenFailure::Unauthorized => {
                        let body = Buffer::from(r#"{"error":"invalid_grant"}"#);
                        let len = body.len() as u64;
                        return Ok(Response::builder()
                            .status(StatusCode::UNAUTHORIZED)
                            .body(HttpBody::new(futures::stream::iter([Ok(body)]), Some(len)))
                            .unwrap());
                    }
                }
            }
            let body = Buffer::from(
                r#"{"access_token":"probe-token","token_type":"Bearer","expires_in":3600}"#,
            );
            let len = body.len() as u64;
            return Ok(Response::new(HttpBody::new(
                futures::stream::iter([Ok(body)]),
                Some(len),
            )));
        }

        assert_eq!(req.uri().host(), Some("storage.googleapis.com"));
        assert_eq!(req.method(), http::Method::GET);
        assert_eq!(req.headers()["authorization"], "Bearer probe-token");
        self.list_requests.fetch_add(1, Ordering::SeqCst);
        let body = Buffer::from(r#"{"kind":"storage#objects","items":[],"prefixes":[]}"#);
        let len = body.len() as u64;
        Ok(Response::new(HttpBody::new(
            futures::stream::iter([Ok(body)]),
            Some(len),
        )))
    }
}

fn operator(failure: TokenFailure) -> (Operator, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let token_requests = Arc::new(AtomicUsize::new(0));
    let list_requests = Arc::new(AtomicUsize::new(0));
    let transport = HttpTransporter::new(TokenTransport {
        failure,
        token_requests: token_requests.clone(),
        list_requests: list_requests.clone(),
    });
    // Encodes dummy authorized-user credentials. The transport handles every request.
    let credential = "eyJ0eXBlIjoiYXV0aG9yaXplZF91c2VyIiwiY2xpZW50X2lkIjoicHJvYmUtY2xpZW50IiwiY2xpZW50X3NlY3JldCI6InByb2JlLXNlY3JldCIsInJlZnJlc2hfdG9rZW4iOiJwcm9iZS1yZWZyZXNoIn0=";
    let op = Operator::new(
        services::Gcs::default()
            .bucket("signing-retry-probe")
            .credential(credential)
            .disable_config_load()
            .disable_vm_metadata(),
    )
    .unwrap()
    .with_context(OperationContext::new().with_http_transport(transport))
    .layer(
        RetryLayer::new()
            .with_min_delay(Duration::from_millis(1))
            .with_max_delay(Duration::from_millis(1))
            .with_max_times(1),
    );
    (op, token_requests, list_requests)
}

#[tokio::test]
async fn temporary_token_failure_recovers() {
    let (op, tokens, lists) = operator(TokenFailure::Temporary);
    let entries = op
        .list("")
        .await
        .expect("temporary token failure must retry");
    assert!(entries.is_empty());
    assert_eq!(tokens.load(Ordering::SeqCst), 2);
    assert_eq!(lists.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn permanent_token_failure_is_not_retried() {
    let (op, tokens, lists) = operator(TokenFailure::Permanent);
    let err = op.list("").await.expect_err("permanent failure must stop");
    assert_eq!(err.kind(), ErrorKind::Unexpected);
    assert!(!err.is_temporary());
    assert_eq!(tokens.load(Ordering::SeqCst), 1);
    assert_eq!(lists.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn rejected_credentials_are_not_retried() {
    let (op, tokens, lists) = operator(TokenFailure::Unauthorized);
    let err = op.list("").await.expect_err("rejected token must stop");
    assert_eq!(err.kind(), ErrorKind::Unexpected);
    assert!(!err.is_temporary());
    assert_eq!(tokens.load(Ordering::SeqCst), 1);
    assert_eq!(lists.load(Ordering::SeqCst), 0);
}
