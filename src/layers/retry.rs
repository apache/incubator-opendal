// Copyright 2022 Datafuse Labs.
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

use std::fmt::Debug;
use std::fmt::Formatter;
use std::sync::Arc;
use std::thread::sleep;

use async_trait::async_trait;
use backon::Backoff;
use backon::Retryable;
use log::warn;

use crate::raw::*;
use crate::*;

/// RetryLayer will add retry for OpenDAL.
///
/// # Examples
///
/// ```
/// use anyhow::Result;
/// use backon::ExponentialBackoff;
/// use opendal::layers::RetryLayer;
/// use opendal::Operator;
/// use opendal::Scheme;
///
/// let _ = Operator::from_env(Scheme::Fs)
///     .expect("must init")
///     .layer(RetryLayer::new(ExponentialBackoff::default()));
/// ```
pub struct RetryLayer<B: Backoff + Send + Sync + Debug + 'static>(B);

impl<B> RetryLayer<B>
where
    B: Backoff + Send + Sync + Debug + 'static,
{
    /// Create a new retry layer.
    /// # Examples
    ///
    /// ```
    /// use anyhow::Result;
    /// use backon::ExponentialBackoff;
    /// use opendal::layers::RetryLayer;
    /// use opendal::Operator;
    /// use opendal::Scheme;
    ///
    /// let _ = Operator::from_env(Scheme::Fs)
    ///     .expect("must init")
    ///     .layer(RetryLayer::new(ExponentialBackoff::default()));
    /// ```
    pub fn new(b: B) -> Self {
        Self(b)
    }
}

impl<B> Layer for RetryLayer<B>
where
    B: Backoff + Send + Sync + Debug + 'static,
{
    fn layer(&self, inner: Arc<dyn Accessor>) -> Arc<dyn Accessor> {
        Arc::new(RetryAccessor {
            inner,
            backoff: self.0.clone(),
        })
    }
}

#[derive(Clone)]
struct RetryAccessor<B: Backoff + Debug + Send + Sync> {
    inner: Arc<dyn Accessor>,
    backoff: B,
}

impl<B: Backoff + Debug + Send + Sync> Debug for RetryAccessor<B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryAccessor")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<B> Accessor for RetryAccessor<B>
where
    B: Backoff + Debug + Send + Sync + 'static,
{
    fn inner(&self) -> Option<Arc<dyn Accessor>> {
        Some(self.inner.clone())
    }

    async fn create(&self, path: &str, args: OpCreate) -> Result<RpCreate> {
        { || self.inner.create(path, args.clone()) }
            .retry(self.backoff.clone())
            .when(|e| e.is_temporary())
            .notify(|err, dur| {
                warn!(
                    target: "opendal::service",
                    "operation={} -> retry after {}s: error={:?}",
                    Operation::Create, dur.as_secs_f64(), err)
            })
            .await
            .map_err(|e| e.set_persistent())
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, output::Reader)> {
        let (rp, r) = { || self.inner.read(path, args.clone()) }
            .retry(self.backoff.clone())
            .when(|e| e.is_temporary())
            .notify(|err, dur| {
                warn!(
                    target: "opendal::service",
                    "operation={} -> retry after {}s: error={:?}",
                    Operation::Read, dur.as_secs_f64(), err)
            })
            .await
            .map_err(|e| e.set_persistent())?;

        Ok((rp, r))
    }

    /// Return `Interrupted` Error even after retry.
    ///
    /// Allowing users to retry the write request from upper logic.
    async fn write(&self, path: &str, args: OpWrite, r: input::Reader) -> Result<RpWrite> {
        self.inner.write(path, args, r).await
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        { || self.inner.stat(path, args.clone()) }
            .retry(self.backoff.clone())
            .when(|e| e.is_temporary())
            .notify(|err, dur| {
                warn!(
                    target: "opendal::service",
                    "operation={} -> retry after {}s: error={:?}",
                    Operation::Stat, dur.as_secs_f64(), err)
            })
            .await
            .map_err(|e| e.set_persistent())
    }

    async fn delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        { || self.inner.delete(path, args.clone()) }
            .retry(self.backoff.clone())
            .when(|e| e.is_temporary())
            .notify(|err, dur| {
                warn!(
                    target: "opendal::service",
                    "operation={} -> retry after {}s: error={:?}",
                    Operation::Delete, dur.as_secs_f64(), err)
            })
            .await
            .map_err(|e| e.set_persistent())
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, ObjectPager)> {
        { || self.inner.list(path, args.clone()) }
            .retry(self.backoff.clone())
            .when(|e| e.is_temporary())
            .notify(|err, dur| {
                warn!(
                    target: "opendal::service",
                    "operation={} -> retry after {}s: error={:?}",
                    Operation::List, dur.as_secs_f64(), err)
            })
            .await
            .map_err(|e| e.set_persistent())
    }

    async fn create_multipart(
        &self,
        path: &str,
        args: OpCreateMultipart,
    ) -> Result<RpCreateMultipart> {
        { || self.inner.create_multipart(path, args.clone()) }
            .retry(self.backoff.clone())
            .when(|e| e.is_temporary())
            .notify(|err, dur| {
                warn!(
                    target: "opendal::service",
                    "operation={} -> retry after {}s: error={:?}",
                    Operation::CreateMultipart, dur.as_secs_f64(), err)
            })
            .await
            .map_err(|e| e.set_persistent())
    }

    async fn write_multipart(
        &self,
        path: &str,
        args: OpWriteMultipart,
        r: input::Reader,
    ) -> Result<RpWriteMultipart> {
        // Write can't retry, until can reset this reader.
        self.inner
            .write_multipart(path, args.clone(), r)
            .await
            .map_err(|e| e.set_persistent())
    }

    async fn complete_multipart(
        &self,
        path: &str,
        args: OpCompleteMultipart,
    ) -> Result<RpCompleteMultipart> {
        { || self.inner.complete_multipart(path, args.clone()) }
            .retry(self.backoff.clone())
            .when(|e| e.is_temporary())
            .notify(|err, dur| {
                warn!(
                    target: "opendal::service",
                    "operation={} -> retry after {}s: error={:?}",
                    Operation::CompleteMultipart, dur.as_secs_f64(), err)
            })
            .await
            .map_err(|e| e.set_persistent())
    }

    async fn abort_multipart(
        &self,
        path: &str,
        args: OpAbortMultipart,
    ) -> Result<RpAbortMultipart> {
        { || self.inner.abort_multipart(path, args.clone()) }
            .retry(self.backoff.clone())
            .when(|e| e.is_temporary())
            .notify(|err, dur| {
                warn!(
                    target: "opendal::service",
                    "operation={} -> retry after {}s: error={:?}",
                    Operation::AbortMultipart, dur.as_secs_f64(), err)
            })
            .await
            .map_err(|e| e.set_persistent())
    }

    fn blocking_create(&self, path: &str, args: OpCreate) -> Result<RpCreate> {
        let retry = self.backoff.clone();

        let mut e = None;

        for dur in retry {
            let res = self.inner.blocking_create(path, args.clone());

            match res {
                Ok(v) => return Ok(v),
                Err(err) => {
                    let retryable = err.is_temporary();
                    e = Some(err);

                    if retryable {
                        sleep(dur);
                        warn!(
                            target: "opendal::service",
                            "operation={} path={} -> retry after {}s: error={:?}",
                            Operation::BlockingCreate, path, dur.as_secs_f64(), e);
                        continue;
                    } else {
                        return Err(e.unwrap());
                    }
                }
            }
        }

        Err(e.unwrap())
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, output::BlockingReader)> {
        let retry = self.backoff.clone();

        let mut e = None;

        for dur in retry {
            let res = self.inner.blocking_read(path, args.clone());

            match res {
                Ok(v) => return Ok(v),
                Err(err) => {
                    let retryable = err.is_temporary();
                    e = Some(err);

                    if retryable {
                        sleep(dur);
                        warn!(
                            target: "opendal::service",
                            "operation={} path={} -> retry after {}s: error={:?}",
                            Operation::BlockingRead, path, dur.as_secs_f64(), e);
                        continue;
                    } else {
                        return Err(e.unwrap());
                    }
                }
            }
        }

        Err(e.unwrap())
    }

    fn blocking_write(
        &self,
        path: &str,
        args: OpWrite,
        r: input::BlockingReader,
    ) -> Result<RpWrite> {
        self.inner.blocking_write(path, args, r)
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let retry = self.backoff.clone();

        let mut e = None;

        for dur in retry {
            let res = self.inner.blocking_stat(path, args.clone());

            match res {
                Ok(v) => return Ok(v),
                Err(err) => {
                    let retryable = err.is_temporary();
                    e = Some(err);

                    if retryable {
                        sleep(dur);
                        warn!(
                            target: "opendal::service",
                            "operation={} path={} -> retry after {}s: error={:?}",
                            Operation::BlockingStat, path, dur.as_secs_f64(), e);
                        continue;
                    } else {
                        return Err(e.unwrap());
                    }
                }
            }
        }

        Err(e.unwrap())
    }

    fn blocking_delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        let retry = self.backoff.clone();

        let mut e = None;

        for dur in retry {
            let res = self.inner.blocking_delete(path, args.clone());

            match res {
                Ok(v) => return Ok(v),
                Err(err) => {
                    let retryable = err.is_temporary();
                    e = Some(err);

                    if retryable {
                        sleep(dur);
                        warn!(
                            target: "opendal::service",
                            "operation={} path={} -> retry after {}s: error={:?}",
                            Operation::BlockingDelete, path, dur.as_secs_f64(), e);
                        continue;
                    } else {
                        return Err(e.unwrap());
                    }
                }
            }
        }

        Err(e.unwrap())
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, BlockingObjectPager)> {
        let retry = self.backoff.clone();

        let mut e = None;

        for dur in retry {
            let res = self.inner.blocking_list(path, args.clone());

            match res {
                Ok(v) => return Ok(v),
                Err(err) => {
                    let retryable = err.is_temporary();
                    e = Some(err);

                    if retryable {
                        sleep(dur);
                        warn!(
                            target: "opendal::service",
                            "operation={} path={} -> retry after {}s: error={:?}",
                            Operation::BlockingList, path, dur.as_secs_f64(), e);
                        continue;
                    } else {
                        return Err(e.unwrap());
                    }
                }
            }
        }

        Err(e.unwrap())
    }
}
