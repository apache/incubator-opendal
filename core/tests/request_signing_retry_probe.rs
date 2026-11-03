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
    // The generated key belongs to no Google account. All HTTP stays in the test transport.
    let credential = "eyJ0eXBlIjoic2VydmljZV9hY2NvdW50IiwicHJvamVjdF9pZCI6Im9wZW5kYWwtc2lnbmluZy1wcm9iZSIsInByaXZhdGVfa2V5X2lkIjoidGFzay1vbmx5LWR1bW15LWtleSIsInByaXZhdGVfa2V5IjoiLS0tLS1CRUdJTiBQUklWQVRFIEtFWS0tLS0tXG5NSUlFdlFJQkFEQU5CZ2txaGtpRzl3MEJBUUVGQUFTQ0JLY3dnZ1NqQWdFQUFvSUJBUURleHJjWGNUL2JYNkRpXG5ndUZUdEk4dVVpREhyYUY0V1lHbVBPa3A1NjNaRGtGaUhNUkpEYlB2RjdLbWFDNTJnd05kOHc5b05hM1pGSCt4XG5mTFBHdUEyN2t2d3gxcTZxTFU2QmdPK1padXRLeUlNQm9iMGx2Q2lxZThrQTZZaGVnVkJZK3A2RmRkcTVCQUFPXG53MDRqOS9DM3R0V0FvRzFxLzZGYi9MOUlScFpLMzZsUFg4cnZJbW1xaXRCZWFXM00zck92TVdKaTVac3FYSGljXG45eWIrTVo5RG9QR3o4elFXaFJjZk5XdWlUYk42eHp5aTY2blNVTkdYOFdwb25sYk5jejUyZzJPZFpGSmw4bW9wXG5QOEdMNk9VRjVRTlZGeVA3VkxlckVGT3MyOWF3L2VLak1XMFhTY0VkVllmNGppRVdpYjJKb3Q4SjNPWVhQS2o2XG4yaVpzclNNbkFnTUJBQUVDZ2dFQVQyOXE3TVI0Z1JnZHJYYitzcEt3OVZMYWxtaXVxVk85RzVJZmo4NHBHcnJ2XG5ESFYyZGhnWlAxbGpYZHZMS1gzNHp1Q3k3elVveXIxNDlNZ2k4R3BEbDAxSGVOdjBod0RGdS83R01KeFdyT0ZIXG5lV2I3STMvdXZpK0tValZ6WjBOVDY0NWR4UGZycUVvcFhSbzkvQ3hXSEtQb2lFZHNESm1mSjBZOXpKOUN5dXBBXG5MVHhZa3N1c3k5bGhpeFB5Q0xzSzJvK2YrVzZWczVuNk9GYmo5UXNvdzBWUDlZeFdhaWZaS1U3NmFkTURuYjVRXG5SaUZIK3JnUUQ2Rk93NUxvenY5dkM1UWhOSk53dlpuVHlpcXJLcWxXY2EwR0daa0EwemhEa0pSa20zYjB4aVhWXG5kM2JGeEh5bTluNVN4dFNJaXFuVTRsenpOcG1RK1l2YXQwRmMyWnlXVFFLQmdRRDRMc1BSa3hjMTd6cWZodk85XG5TOWtWYThFVjVUclM5MmprYm9wRWlHMWdWblZxU2lqNTBVWWFhVDJoTGN0aDJlMG1LR1NwM1o4N1ZHb0VwVkZDXG5QYSt4UDhHQUNkY1hHbG56S3E0SVZMMElMQlB1K1p6SFpldUIrUWl1U0ZNNnF2Z0k0dDVyL1pNSU1CMGpzY0JUXG5vM0FaRDJXWit2V0Y5NlgrYUF5dU42S1Znd0tCZ1FEbHl4Vi93aWJlSXBydjk1R1hKWnVZZVAzMDJkWHVscWZjXG5RdUIvNDIzMHZ2anFLdm9Ib2FkRGQ0ZGlybXhMYmpWRldDbEI1THRmUXpPaHc2ZTlNUSs1NWh4Zm5IWkFEZ1JwXG5DYlNTcXNHeWl2VkZoQ1dnVDFweDQ1R0hNUW9TYzFYWGx3SzMrVGpzRUFiQjZ1T0ZwaC9FQ1ZiNjZNaDU3L3IzXG5kdjRoVVl2dWpRS0JnUURpbmNFTVlxVTFDR2FkT0JyajVKZ3FhU1A4SEQ0a1dPbWNmQnNoRFBuaWw4L0s2RUNXXG5MRE00OXBQaDB2RDlQb0srN3U5TDlDUHJodDd2c3d5SjBjM1ZxWVBFcURlUXp1MEhqeGN1WUVIamJtd2RNQ3VxXG5IelIyajdFUGd0Y01vUVN2Q0lsVVRreXRrMUZvUUd4L2NxTnZIcElGa2hRUzZkaldmcWZsaE1PZWhRS0JnRDY5XG5ETGlkNUlpL2YvZkdhdXlPYXdYYyt0RXY3RlhBc2tIVHpJRitNWlloMGNuMVErQnpoVHhKVlNCZktOZ0xWZVlCXG41UG5GbmxhcS9Wek1FMjczQnlrbEoxZi9XdFd3UGdvRGlWYUY0bTZCdStkbjVmSDMyR1RCMGdqTUhjamR6bkVYXG5ITG5vQkYrSEhLRXpKVDFuRnNNcUhhSlJqZktrejZ0aXhJMktPeDg5QW9HQURKQ2E0STVRYmovOXk5R1Q4eG9YXG52L1hJalkrWlpQMU00R0JyTmxGOC9EcDJFUThlVnhXSE9SSU5FTW5CbVFqYmU0ZXJhYXByRVI5QTdiK2RoeklpXG5ZWmFHOE1yb3ZkRkV4ZURlVWpIN1hSNFVoc3BvbktkZXEzaUVjUkR1Y1oydjJrZHpsMkp1N3lqb2RocWMvWDJSXG56RHN6NCtWZFdHNHI0MnlqWVNlanRzcz1cbi0tLS0tRU5EIFBSSVZBVEUgS0VZLS0tLS1cbiIsImNsaWVudF9lbWFpbCI6Im9wZW5kYWwtc2lnbmluZy1wcm9iZUBleGFtcGxlLmludmFsaWQiLCJjbGllbnRfaWQiOiIxMjM0NTY3ODkwIiwiYXV0aF91cmkiOiJodHRwczovL2FjY291bnRzLmdvb2dsZS5jb20vby9vYXV0aDIvYXV0aCIsInRva2VuX3VyaSI6Imh0dHBzOi8vb2F1dGgyLmdvb2dsZWFwaXMuY29tL3Rva2VuIn0=";
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
