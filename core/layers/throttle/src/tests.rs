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

use std::future::poll_fn;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Poll;
use std::time::Duration;

use super::*;
use opendal_core::raw::oio::{Read as _, ReadStream as _};

fn test_limiter(burst: u32) -> SharedRateLimiter {
    Arc::new(RateLimiter::direct(
        Quota::with_period(Duration::from_secs(3600))
            .unwrap()
            .allow_burst(NonZeroU32::new(burst).unwrap()),
    ))
}

struct CountingReader {
    calls: Arc<AtomicUsize>,
}

impl CountingReader {
    fn data(range: BytesRange) -> Buffer {
        let start = range.offset() as usize;
        let end = range.size().map_or(6, |size| start + size as usize);
        Buffer::from("abcdef").slice(start..end)
    }
}

impl oio::Read for CountingReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok((
            RpRead::new(MetadataBuilder::file(6).build()),
            Box::new(Self::data(range)),
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok((
            RpRead::new(MetadataBuilder::file(6).build()),
            Self::data(range),
        ))
    }
}

#[tokio::test]
async fn bounded_reads_check_quota_before_inner_io() {
    let calls = Arc::new(AtomicUsize::new(0));
    let reader = ThrottleWrapper::new(
        CountingReader {
            calls: calls.clone(),
        },
        test_limiter(4),
    );
    let error = reader
        .read(BytesRange::new(0, Some(5)))
        .await
        .expect_err("oversized read must fail");
    assert_eq!(error.kind(), ErrorKind::RateLimited);
    let error = reader
        .open(BytesRange::new(0, Some(5)))
        .await
        .err()
        .expect("oversized stream must fail");
    assert_eq!(error.kind(), ErrorKind::RateLimited);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let (_, data) = reader.read(BytesRange::new(0, Some(4))).await.unwrap();
    assert_eq!(data.to_vec(), b"abcd");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let mut request = Box::pin(reader.open(BytesRange::new(0, Some(1))));
    poll_fn(|cx| {
        assert!(std::future::Future::poll(request.as_mut(), cx).is_pending());
        Poll::Ready(())
    })
    .await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unbounded_streams_remain_unthrottled() {
    let calls = Arc::new(AtomicUsize::new(0));
    let limiter = test_limiter(4);
    let reader = ThrottleWrapper::new(
        CountingReader {
            calls: calls.clone(),
        },
        limiter.clone(),
    );
    let (_, mut stream) = reader.open(BytesRange::new(0, Some(4))).await.unwrap();
    assert_eq!(stream.read_all().await.unwrap().to_vec(), b"abcd");
    assert!(limiter.check().is_err());

    let (_, mut stream) = reader.open(BytesRange::default()).await.unwrap();
    assert_eq!(stream.read_all().await.unwrap().to_vec(), b"abcdef");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn public_memory_bounded_and_chunked_reads_share_quota() {
    let base = Operator::new(services::Memory::default()).unwrap();
    base.write("data", "abcdef").await.unwrap();
    let limiter = test_limiter(20);
    let op = base.layer(ThrottleLayer {
        rate_limiter: limiter.clone(),
    });

    assert_eq!(
        op.read_with("data").range(1..5).await.unwrap().to_vec(),
        b"bcde"
    );
    assert_eq!(
        op.read_with("data")
            .range(0..6)
            .chunk(2)
            .await
            .unwrap()
            .to_vec(),
        b"abcdef"
    );
    for _ in 0..10 {
        assert!(limiter.check().is_ok());
    }
    assert!(limiter.check().is_err());
}

#[tokio::test]
async fn public_memory_rejects_oversized_bounded_reads() {
    let base = Operator::new(services::Memory::default()).unwrap();
    base.write("data", "abcdef").await.unwrap();
    assert_eq!(base.read("data").await.unwrap().to_vec(), b"abcdef");
    let op = base.layer(ThrottleLayer::new(1024, 4));
    let error = op
        .read_with("data")
        .range(0..6)
        .await
        .expect_err("oversized range must fail");
    assert_eq!(error.kind(), ErrorKind::RateLimited);
}

#[tokio::test]
async fn public_memory_writes_keep_buffer_quota() {
    let base = Operator::new(services::Memory::default()).unwrap();
    let limiter = test_limiter(20);
    let op = base.clone().layer(ThrottleLayer {
        rate_limiter: limiter.clone(),
    });
    op.write("data", "abcd").await.unwrap();
    op.write("empty", "").await.unwrap();
    assert_eq!(base.read("data").await.unwrap().to_vec(), b"abcd");
    assert!(base.read("empty").await.unwrap().is_empty());
    for _ in 0..16 {
        assert!(limiter.check().is_ok());
    }
    assert!(limiter.check().is_err());
}
