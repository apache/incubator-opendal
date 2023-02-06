// Copyright 2023 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io;
use std::task::Context;
use std::task::Poll;

use async_trait::async_trait;
use bytes::Bytes;
use futures::FutureExt;
use rand::prelude::*;
use rand::rngs::StdRng;

use crate::raw::*;
use crate::*;

/// Inject chaos into underlying services for robustness test.
///
/// # Chaos
///
/// Chaos tests is a part of stress test. By generating errors at specified
/// error ratio, we can reproduce underlying services error more reliable.
///
/// Running tests under ChaosLayer will make your application more robust.
///
/// For example: If we specify an error rate of 0.5, there is a 50% chance
/// of an EOF error for every read operation.
///
/// # Note
///
/// For now, ChaosLayer only injects read operations. More operations may
/// be added in the future.
///
/// # Examples
///
/// ```
/// use anyhow::Result;
/// use opendal::layers::ChaosLayer;
/// use opendal::services;
/// use opendal::Operator;
/// use opendal::Scheme;
///
/// let _ = Operator::from_env::<services::Fs>()
///     .expect("must init")
///     .layer(ChaosLayer::new(0.1))
///     .finish();
/// ```
#[derive(Debug, Clone)]
pub struct ChaosLayer {
    error_ratio: f64,
}

impl ChaosLayer {
    /// Create a new chaos layer with specified error ratio.
    ///
    /// # Panics
    ///
    /// Input error_ratio must in [0.0..=1.0]
    pub fn new(error_ratio: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&error_ratio),
            "error_ratio must between 0.0 and 1.0"
        );
        Self { error_ratio }
    }
}

impl<A: Accessor> Layer<A> for ChaosLayer {
    type LayeredAccessor = ChaosAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccessor {
        ChaosAccessor {
            inner,
            rng: StdRng::from_entropy(),
            error_ratio: self.error_ratio,
        }
    }
}

#[derive(Debug)]
pub struct ChaosAccessor<A> {
    inner: A,
    rng: StdRng,

    error_ratio: f64,
}

#[async_trait]
impl<A: Accessor> LayeredAccessor for ChaosAccessor<A> {
    type Inner = A;
    type Reader = ChaosReader<A::Reader>;
    type BlockingReader = ChaosReader<A::BlockingReader>;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        self.inner
            .read(path, args)
            .map(|v| v.map(|(rp, r)| (rp, ChaosReader::new(r, self.rng.clone(), self.error_ratio))))
            .await
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        self.inner
            .blocking_read(path, args)
            .map(|(rp, r)| (rp, ChaosReader::new(r, self.rng.clone(), self.error_ratio)))
    }
}

/// ChaosReader will inject error into read operations.
pub struct ChaosReader<R> {
    inner: R,
    rng: StdRng,

    error_ratio: f64,
}

impl<R> ChaosReader<R> {
    fn new(inner: R, rng: StdRng, error_ratio: f64) -> Self {
        Self {
            inner,
            rng,
            error_ratio,
        }
    }

    /// If I feel lucky, we can return the correct response. Otherwise,
    /// we need to generate an error.
    fn i_feel_lucky(&mut self) -> bool {
        let point = self.rng.gen_range(0..=100);
        (self.error_ratio * 100.0) as i32 >= point
    }

    fn unexpected_eof() -> io::Error {
        io::Error::new(
            io::ErrorKind::UnexpectedEof,
            Error::new(ErrorKind::Unexpected, "I am your chaos!").set_temporary(),
        )
    }
}

impl<R: output::Read> output::Read for ChaosReader<R> {
    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        if self.i_feel_lucky() {
            self.inner.poll_read(cx, buf)
        } else {
            Poll::Ready(Err(Self::unexpected_eof()))
        }
    }

    fn poll_seek(&mut self, cx: &mut Context<'_>, pos: io::SeekFrom) -> Poll<io::Result<u64>> {
        if self.i_feel_lucky() {
            self.inner.poll_seek(cx, pos)
        } else {
            Poll::Ready(Err(Self::unexpected_eof()))
        }
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<io::Result<Bytes>>> {
        if self.i_feel_lucky() {
            self.inner.poll_next(cx)
        } else {
            Poll::Ready(Some(Err(Self::unexpected_eof())))
        }
    }
}

impl<R: output::BlockingRead> output::BlockingRead for ChaosReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.i_feel_lucky() {
            self.inner.read(buf)
        } else {
            Err(Self::unexpected_eof())
        }
    }

    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        if self.i_feel_lucky() {
            self.inner.seek(pos)
        } else {
            Err(Self::unexpected_eof())
        }
    }

    fn next(&mut self) -> Option<io::Result<Bytes>> {
        if self.i_feel_lucky() {
            self.inner.next()
        } else {
            Some(Err(Self::unexpected_eof()))
        }
    }
}
