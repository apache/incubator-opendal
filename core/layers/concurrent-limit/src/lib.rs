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

#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use asyncband::semaphore::OwnedSemaphorePermit;
use asyncband::semaphore::Semaphore;
use futures::Stream;
use futures::StreamExt;
use opendal_core::raw::*;
use opendal_core::*;

/// ConcurrentLimitSemaphore abstracts a semaphore-like concurrency primitive
/// that yields an owned permit released on drop.
pub trait ConcurrentLimitSemaphore: Send + Sync + Clone + Unpin + 'static {
    /// The owned permit type associated with the semaphore. Dropping it
    /// must release the permit back to the semaphore.
    type Permit: Send + Sync + 'static;

    /// Acquire an owned permit asynchronously.
    fn acquire(&self) -> impl Future<Output = Self::Permit> + MaybeSend;
}

impl ConcurrentLimitSemaphore for Arc<Semaphore> {
    type Permit = OwnedSemaphorePermit;

    async fn acquire(&self) -> Self::Permit {
        self.clone().acquire_owned(1).await
    }
}

/// `ConcurrentLimitLayer` controls how many concurrent requests OpenDAL can send
/// to a storage service.
///
/// Operators that reuse the same [`ConcurrentLimitLayer`] instance share a
/// semaphore. This lets an application enforce one total concurrent-request
/// limit across multiple operators.
///
/// # Examples
///
/// The following example adds a concurrent limit layer to an operator:
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_concurrent_limit::ConcurrentLimitLayer;
/// #
/// # fn main() -> Result<()> {
/// let _ = Operator::new(services::Memory::default())?
///     .layer(ConcurrentLimitLayer::new(1024));
/// # Ok(())
/// # }
/// ```
///
/// Share a concurrent limit layer between the operators:
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_concurrent_limit::ConcurrentLimitLayer;
/// #
/// # fn main() -> Result<()> {
/// let limit = ConcurrentLimitLayer::new(1024);
///
/// let _operator_a = Operator::new(services::Memory::default())?
///     .layer(limit.clone());
/// let _operator_b = Operator::new(services::Memory::default())?
///     .layer(limit.clone());
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ConcurrentLimitLayer<S: ConcurrentLimitSemaphore = Arc<Semaphore>> {
    operation_semaphore: S,
    http_semaphore: Option<S>,
}

impl<S: ConcurrentLimitSemaphore> std::fmt::Debug for ConcurrentLimitLayer<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcurrentLimitLayer")
            .field("has_http_limit", &self.http_semaphore.is_some())
            .finish_non_exhaustive()
    }
}

impl ConcurrentLimitLayer<Arc<Semaphore>> {
    /// Create a new `ConcurrentLimitLayer` with the specified number of
    /// permits.
    ///
    /// These permits will be applied to all operations.
    pub fn new(permits: usize) -> Self {
        Self::with_semaphore(Arc::new(Semaphore::new(permits)))
    }

    /// Set a concurrent limit for HTTP requests.
    ///
    /// This convenience helper constructs a new semaphore with the specified
    /// number of permits and calls [`ConcurrentLimitLayer::with_http_semaphore`].
    /// Use [`ConcurrentLimitLayer::with_http_semaphore`] directly when reusing
    /// a shared semaphore.
    pub fn with_http_concurrent_limit(self, permits: usize) -> Self {
        self.with_http_semaphore(Arc::new(Semaphore::new(permits)))
    }
}

impl<S: ConcurrentLimitSemaphore> ConcurrentLimitLayer<S> {
    /// Create a layer with any ConcurrentLimitSemaphore implementation.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use asyncband::semaphore::Semaphore;
    /// # use opendal_layer_concurrent_limit::ConcurrentLimitLayer;
    /// let semaphore = Arc::new(Semaphore::new(1024));
    /// let _layer = ConcurrentLimitLayer::with_semaphore(semaphore);
    /// ```
    pub fn with_semaphore(operation_semaphore: S) -> Self {
        Self {
            operation_semaphore,
            http_semaphore: None,
        }
    }

    /// Provide a custom HTTP concurrency semaphore instance.
    pub fn with_http_semaphore(mut self, semaphore: S) -> Self {
        self.http_semaphore = Some(semaphore);
        self
    }
}

impl<S: ConcurrentLimitSemaphore> Layer for ConcurrentLimitLayer<S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }

    fn apply_context(&self, _srv: Servicer, inner: OperationContext) -> OperationContext {
        // Wrap the current HTTP transport so HTTP permits are held until the
        // response body is dropped.
        let transport = HttpTransporter::new(ConcurrentLimitHttpTransport::<S> {
            inner: inner.http_transport().clone(),
            http_semaphore: self.http_semaphore.clone(),
        });
        inner.with_http_transport(transport)
    }
}

impl<S: ConcurrentLimitSemaphore> ConcurrentLimitLayer<S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    fn layer(&self, inner: Servicer) -> ConcurrentLimitService<S> {
        ConcurrentLimitService {
            inner,
            semaphore: self.operation_semaphore.clone(),
        }
    }
}

#[doc(hidden)]
pub struct ConcurrentLimitHttpTransport<S: ConcurrentLimitSemaphore> {
    inner: HttpTransporter,
    http_semaphore: Option<S>,
}

impl<S: ConcurrentLimitSemaphore> HttpTransport for ConcurrentLimitHttpTransport<S>
where
    S::Permit: Unpin,
{
    async fn fetch(&self, req: http::Request<Buffer>) -> Result<http::Response<HttpBody>> {
        let Some(semaphore) = self.http_semaphore.clone() else {
            return self.inner.fetch(req).await;
        };

        let permit = semaphore.acquire().await;

        let resp = self.inner.fetch(req).await?;
        let (parts, body) = resp.into_parts();
        let body = body.map_inner(|s| {
            Box::new(ConcurrentLimitStream::<_, S::Permit> {
                inner: s,
                _permit: permit,
            })
        });
        Ok(http::Response::from_parts(parts, body))
    }
}

struct ConcurrentLimitStream<S, P> {
    inner: S,
    // Hold this permit until the HTTP body stream is dropped.
    _permit: P,
}

impl<S, P> Stream for ConcurrentLimitStream<S, P>
where
    S: Stream<Item = Result<Buffer>> + Unpin + 'static,
    P: Unpin,
{
    type Item = Result<Buffer>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Safe due to Unpin bounds on S and P (thus on Self).
        let this = self.get_mut();
        this.inner.poll_next_unpin(cx)
    }
}

#[doc(hidden)]
#[derive(Clone)]
pub struct ConcurrentLimitService<S: ConcurrentLimitSemaphore> {
    inner: Servicer,
    semaphore: S,
}

impl<S: ConcurrentLimitSemaphore> std::fmt::Debug for ConcurrentLimitService<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcurrentLimitService")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<S: ConcurrentLimitSemaphore> Service for ConcurrentLimitService<S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    type Reader = ConcurrentLimitReader<oio::Reader, S>;
    type Writer = ConcurrentLimitWrapper<oio::Writer, S>;
    type Lister = ConcurrentLimitWrapper<oio::Lister, S>;
    type Deleter = ConcurrentLimitWrapper<oio::Deleter, S>;
    type Copier = ConcurrentLimitWrapper<oio::Copier, S>;
    type Composer = ConcurrentLimitWrapper<oio::Composer, S>;

    fn info(&self) -> ServiceInfo {
        self.inner.info()
    }

    fn capability(&self) -> Capability {
        self.inner.capability()
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let _permit = self.semaphore.acquire().await;
        self.inner.create_dir(ctx, path, args).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        self.inner
            .read(ctx, path, args)
            .map(|r| ConcurrentLimitReader::new(r, self.semaphore.clone()))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        self.inner
            .write(ctx, path, args)
            .map(|w| ConcurrentLimitWrapper::new(w, self.semaphore.clone()))
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        self.inner
            .copy(ctx, from, to, args)
            .map(|c| ConcurrentLimitWrapper::new(c, self.semaphore.clone()))
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.inner
            .compose(ctx, to, args)
            .map(|c| ConcurrentLimitWrapper::new(c, self.semaphore.clone()))
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        let _permit = self.semaphore.acquire().await;
        self.inner.rename(ctx, from, to, args).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        let _permit = self.semaphore.acquire().await;
        self.inner.restore(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let _permit = self.semaphore.acquire().await;
        self.inner.stat(ctx, path, args).await
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner
            .delete(ctx)
            .map(|w| ConcurrentLimitWrapper::new(w, self.semaphore.clone()))
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        self.inner
            .list(ctx, path, args)
            .map(|s| ConcurrentLimitWrapper::new(s, self.semaphore.clone()))
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let _permit = self.semaphore.acquire().await;
        self.inner.presign(ctx, path, args).await
    }
}

#[doc(hidden)]
pub struct ConcurrentLimitReader<R, S> {
    inner: R,
    semaphore: S,
}

impl<R, S> ConcurrentLimitReader<R, S> {
    fn new(inner: R, semaphore: S) -> Self {
        Self { inner, semaphore }
    }
}

impl<R: oio::Read, S: ConcurrentLimitSemaphore> oio::Read for ConcurrentLimitReader<R, S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let permit = self.semaphore.acquire().await;
        let (rp, stream) = self.inner.open(range).await?;
        Ok((
            rp,
            Box::new(ConcurrentLimitWrapper::new_with_permit(
                stream,
                self.semaphore.clone(),
                permit,
            )) as Box<dyn oio::ReadStreamDyn>,
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let _permit = self.semaphore.acquire().await;
        self.inner.read(range).await
    }
}

#[doc(hidden)]
pub struct ConcurrentLimitWrapper<R, S: ConcurrentLimitSemaphore> {
    inner: R,
    semaphore: S,
    // Hold this permit until the wrapped operation body is dropped.
    permit: Option<S::Permit>,
}

impl<R, S: ConcurrentLimitSemaphore> ConcurrentLimitWrapper<R, S> {
    fn new(inner: R, semaphore: S) -> Self {
        Self {
            inner,
            semaphore,
            permit: None,
        }
    }

    fn new_with_permit(inner: R, semaphore: S, permit: S::Permit) -> Self {
        Self {
            inner,
            semaphore,
            permit: Some(permit),
        }
    }

    async fn acquire(&mut self) {
        if self.permit.is_none() {
            self.permit = Some(self.semaphore.acquire().await);
        }
    }
}

impl<R: oio::ReadStream, S: ConcurrentLimitSemaphore> oio::ReadStream
    for ConcurrentLimitWrapper<R, S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    async fn read(&mut self) -> Result<Buffer> {
        self.acquire().await;
        self.inner.read().await
    }
}

impl<R: oio::Read, S: ConcurrentLimitSemaphore> oio::Read for ConcurrentLimitWrapper<R, S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        self.inner.open(range).await
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        self.inner.read(range).await
    }
}

impl<R: oio::Write, S: ConcurrentLimitSemaphore> oio::Write for ConcurrentLimitWrapper<R, S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.acquire().await;
        self.inner.write(bs).await
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        self.acquire().await;
        self.inner.copy_from(path, args, range).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.acquire().await;
        self.inner.close().await
    }

    async fn abort(&mut self) -> Result<()> {
        self.acquire().await;
        self.inner.abort().await
    }
}

impl<R: oio::List, S: ConcurrentLimitSemaphore> oio::List for ConcurrentLimitWrapper<R, S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.acquire().await;
        self.inner.next().await
    }
}

impl<R: oio::Delete, S: ConcurrentLimitSemaphore> oio::Delete for ConcurrentLimitWrapper<R, S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.acquire().await;
        self.inner.delete(path, args).await
    }

    async fn close(&mut self) -> Result<()> {
        self.acquire().await;
        self.inner.close().await
    }
}

impl<C: oio::Copy, S: ConcurrentLimitSemaphore> oio::Copy for ConcurrentLimitWrapper<C, S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    async fn next(&mut self) -> Result<Option<usize>> {
        self.acquire().await;
        self.inner.next().await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.acquire().await;
        self.inner.close().await
    }

    async fn abort(&mut self) -> Result<()> {
        self.acquire().await;
        self.inner.abort().await
    }
}

impl<C: oio::Compose, S: ConcurrentLimitSemaphore> oio::Compose for ConcurrentLimitWrapper<C, S>
where
    S::Permit: Send + Sync + 'static + Unpin,
{
    async fn compose(&mut self, path: &str, args: OpRead) -> Result<()> {
        self.acquire().await;
        self.inner.compose(path, args).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.acquire().await;
        self.inner.close().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendal_core::Operator;
    use opendal_core::services;
    use std::future::pending;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::timeout;

    use futures::stream;
    use http::Response;

    #[tokio::test]
    async fn operation_semaphore_can_be_shared() {
        let semaphore = Arc::new(Semaphore::new(1));
        let layer = ConcurrentLimitLayer::with_semaphore(semaphore.clone());

        let permit = semaphore.clone().acquire_owned(1).await;

        let op = Operator::new(services::Memory::default())
            .expect("operator must build")
            .layer(layer);

        let blocked = timeout(Duration::from_millis(50), op.stat("any")).await;
        assert!(
            blocked.is_err(),
            "operation should be limited by shared semaphore"
        );

        drop(permit);

        let completed = timeout(Duration::from_millis(50), op.stat("any")).await;
        assert!(
            completed.is_ok(),
            "operation should proceed once permit is released"
        );
    }

    #[tokio::test]
    async fn operation_semaphore_limits_copy_and_rename() {
        #[derive(Clone, Debug)]
        struct CopyRenameBackend {
            info: ServiceInfo,
            capability: Capability,
        }

        impl Service for CopyRenameBackend {
            type Reader = ();
            type Writer = ();
            type Lister = ();
            type Deleter = ();
            type Copier = ();
            type Composer = ();

            fn info(&self) -> ServiceInfo {
                self.info.clone()
            }

            fn capability(&self) -> Capability {
                self.capability
            }

            async fn create_dir(
                &self,
                _: &OperationContext,
                _: &str,
                _: OpCreateDir,
            ) -> Result<RpCreateDir> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn read(&self, _ctx: &OperationContext, _: &str, _: OpRead) -> Result<Self::Reader> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn write(&self, _: &OperationContext, _: &str, _: OpWrite) -> Result<Self::Writer> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn list(&self, _ctx: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn copy(
                &self,
                _: &OperationContext,
                _: &str,
                _: &str,
                _: OpCopy,
            ) -> Result<Self::Copier> {
                Ok(())
            }

            async fn rename(
                &self,
                _: &OperationContext,
                _: &str,
                _: &str,
                _: OpRename,
            ) -> Result<RpRename> {
                Ok(RpRename::default())
            }

            async fn presign(
                &self,
                _: &OperationContext,
                _: &str,
                _: OpPresign,
            ) -> Result<RpPresign> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }
        }

        let semaphore = Arc::new(Semaphore::new(1));
        let layer = ConcurrentLimitLayer::with_semaphore(semaphore.clone());
        let capability = Capability {
            copy: true,
            rename: true,
            ..Default::default()
        };
        let op = Operator::from_parts(
            OperationContext::default(),
            Arc::new(CopyRenameBackend {
                info: ServiceInfo::with_scheme("mock"),
                capability,
            }),
        )
        .layer(layer);

        let permit = semaphore.clone().acquire_owned(1).await;

        let copy = timeout(Duration::from_millis(50), op.copy("from", "to")).await;
        assert!(copy.is_err(), "copy should wait for the operation permit");

        let rename = timeout(Duration::from_millis(50), op.rename("from", "to")).await;
        assert!(
            rename.is_err(),
            "rename should wait for the operation permit"
        );

        drop(permit);

        timeout(Duration::from_millis(50), op.copy("from", "to"))
            .await
            .expect("copy should proceed once permit is released")
            .expect("copy should succeed");
        timeout(Duration::from_millis(50), op.rename("from", "to"))
            .await
            .expect("rename should proceed once permit is released")
            .expect("rename should succeed");
    }

    #[tokio::test]
    async fn operation_semaphore_held_until_copier_dropped() {
        #[derive(Debug)]
        struct PendingCopier;

        impl oio::Copy for PendingCopier {
            async fn next(&mut self) -> Result<Option<usize>> {
                pending().await
            }

            async fn close(&mut self) -> Result<Metadata> {
                pending().await
            }

            async fn abort(&mut self) -> Result<()> {
                Ok(())
            }
        }

        #[derive(Clone, Debug)]
        struct CopierBackend {
            info: ServiceInfo,
            capability: Capability,
        }

        impl Service for CopierBackend {
            type Reader = ();
            type Writer = ();
            type Lister = ();
            type Deleter = ();
            type Copier = PendingCopier;
            type Composer = ();

            fn info(&self) -> ServiceInfo {
                self.info.clone()
            }

            fn capability(&self) -> Capability {
                self.capability
            }

            async fn create_dir(
                &self,
                _: &OperationContext,
                _: &str,
                _: OpCreateDir,
            ) -> Result<RpCreateDir> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn read(&self, _ctx: &OperationContext, _: &str, _: OpRead) -> Result<Self::Reader> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn write(&self, _: &OperationContext, _: &str, _: OpWrite) -> Result<Self::Writer> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn list(&self, _ctx: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn copy(
                &self,
                _: &OperationContext,
                _: &str,
                _: &str,
                _: OpCopy,
            ) -> Result<Self::Copier> {
                Ok(PendingCopier)
            }

            async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
                Ok(RpStat::new(MetadataBuilder::file(0).build()))
            }

            async fn rename(
                &self,
                _: &OperationContext,
                _: &str,
                _: &str,
                _: OpRename,
            ) -> Result<RpRename> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            async fn presign(
                &self,
                _: &OperationContext,
                _: &str,
                _: OpPresign,
            ) -> Result<RpPresign> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }
        }

        let semaphore = Arc::new(Semaphore::new(1));
        let layer = ConcurrentLimitLayer::with_semaphore(semaphore.clone());
        let capability = Capability {
            copy: true,
            stat: true,
            ..Default::default()
        };
        let op = Operator::from_parts(
            OperationContext::default(),
            Arc::new(CopierBackend {
                info: ServiceInfo::with_scheme("mock"),
                capability,
            }),
        )
        .layer(layer);

        let mut copier = timeout(Duration::from_millis(50), op.copier("from", "to"))
            .await
            .expect("copier setup should not block")
            .expect("copier should be created");

        let copy = timeout(Duration::from_millis(50), copier.next()).await;
        assert!(copy.is_err(), "copy body should remain pending");

        // The permit is held by the active copy body, so concurrent operations
        // must time out until the copier is dropped.
        let blocked = timeout(Duration::from_millis(50), op.stat("any")).await;
        assert!(
            blocked.is_err(),
            "stat should wait while the copier holds the permit"
        );

        drop(copier);

        timeout(Duration::from_millis(50), op.stat("any"))
            .await
            .expect("stat should proceed once the copier is dropped")
            .expect("stat should succeed");
    }

    #[tokio::test]
    async fn concurrent_chunked_read_with_http_limit() {
        use opendal_core::raw::*;

        struct EchoTransport;

        impl HttpTransport for EchoTransport {
            async fn fetch(&self, req: http::Request<Buffer>) -> Result<http::Response<HttpBody>> {
                let data = req.into_body();
                let len = data.len() as u64;
                let body =
                    HttpBody::new(Box::pin(stream::once(async move { Ok(data) })), Some(len));
                Ok(http::Response::builder()
                    .status(http::StatusCode::OK)
                    .body(body)
                    .unwrap())
            }
        }

        #[derive(Clone, Debug)]
        struct HttpBackend {
            info: ServiceInfo,
            capability: Capability,
            content: Buffer,
        }

        /// Reader returned by this backend.
        pub struct HttpReader {
            backend: HttpBackend,
            ctx: OperationContext,
        }

        impl HttpReader {
            fn new(backend: HttpBackend, ctx: OperationContext, _: &str, _: OpRead) -> Self {
                Self { backend, ctx }
            }
        }

        impl oio::StreamRead for HttpReader {
            async fn open(
                &self,
                range: BytesRange,
            ) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
                let backend = &self.backend;
                let start = range.offset() as usize;
                let data = match range.size() {
                    Some(sz) => backend.content.slice(start..start + sz as usize),
                    None => backend.content.slice(start..),
                };
                let req = http::Request::get("http://fake").body(data).unwrap();
                let resp = self.ctx.http_transport().fetch(req).await?;
                let rp = RpRead::new({
                    let metadata = MetadataBuilder::file(backend.content.len() as u64);
                    metadata.build()
                });
                let stream = resp.into_body();

                Ok((rp, Box::new(stream) as Box<dyn oio::ReadStreamDyn>))
            }
        }

        impl Service for HttpBackend {
            type Reader = oio::StreamReader<HttpReader>;
            type Writer = ();
            type Lister = ();
            type Deleter = ();
            type Copier = ();
            type Composer = ();

            fn info(&self) -> ServiceInfo {
                self.info.clone()
            }

            fn capability(&self) -> Capability {
                self.capability
            }

            async fn create_dir(
                &self,
                _: &OperationContext,
                _: &str,
                _: OpCreateDir,
            ) -> Result<RpCreateDir> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn read(
                &self,
                ctx: &OperationContext,
                path: &str,
                args: OpRead,
            ) -> Result<Self::Reader> {
                Ok(oio::StreamReader::new(HttpReader::new(
                    self.clone(),
                    ctx.clone(),
                    path,
                    args,
                )))
            }

            async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
                Ok(RpStat::new({
                    let metadata = MetadataBuilder::file(self.content.len() as u64);
                    metadata.build()
                }))
            }

            fn write(&self, _: &OperationContext, _: &str, _: OpWrite) -> Result<Self::Writer> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn delete(&self, _ctx: &OperationContext) -> Result<Self::Deleter> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn list(&self, _ctx: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            fn copy(
                &self,
                _: &OperationContext,
                _: &str,
                _: &str,
                _: OpCopy,
            ) -> Result<Self::Copier> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            async fn rename(
                &self,
                _: &OperationContext,
                _: &str,
                _: &str,
                _: OpRename,
            ) -> Result<RpRename> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }

            async fn presign(
                &self,
                _: &OperationContext,
                _: &str,
                _: OpPresign,
            ) -> Result<RpPresign> {
                Err(Error::new(
                    ErrorKind::Unsupported,
                    "operation is not supported",
                ))
            }
        }

        let content = Buffer::from(vec![0u8; 4096]);
        let op = Operator::from_parts(
            OperationContext::default(),
            Arc::new(HttpBackend {
                info: ServiceInfo::with_scheme("mock"),
                capability: Capability {
                    read: true,
                    stat: true,
                    ..Default::default()
                },
                content: content.clone(),
            }),
        )
        .with_context(
            OperationContext::new().with_http_transport(HttpTransporter::new(EchoTransport)),
        )
        .layer(ConcurrentLimitLayer::new(1024).with_http_concurrent_limit(2));

        // chunk=256 ⇒ 16 HTTP requests, concurrent=4, but only 2 HTTP permits.
        let result = timeout(Duration::from_secs(5), async {
            op.reader_with("test")
                .chunk(256)
                .concurrent(4)
                .await
                .expect("reader must build")
                .read(..)
                .await
        })
        .await;

        let buf = result
            .expect("read must not deadlock (timeout)")
            .expect("read must succeed");
        assert_eq!(buf.to_bytes(), content.to_bytes());
    }

    #[tokio::test]
    async fn http_semaphore_holds_until_body_dropped() {
        struct DummyTransport;

        impl HttpTransport for DummyTransport {
            async fn fetch(&self, _req: http::Request<Buffer>) -> Result<Response<HttpBody>> {
                let body = HttpBody::new(stream::empty(), None);
                Ok(Response::builder()
                    .status(http::StatusCode::OK)
                    .body(body)
                    .expect("response must build"))
            }
        }

        let semaphore = Arc::new(Semaphore::new(1));
        let layer = ConcurrentLimitLayer::new(1).with_http_semaphore(semaphore.clone());
        let fetcher = ConcurrentLimitHttpTransport::<Arc<Semaphore>> {
            inner: HttpTransporter::new(DummyTransport),
            http_semaphore: layer.http_semaphore.clone(),
        };

        let request = http::Request::builder()
            .uri("http://example.invalid/")
            .body(Buffer::new())
            .expect("request must build");
        let _resp = fetcher
            .fetch(request)
            .await
            .expect("first fetch should succeed");

        let request = http::Request::builder()
            .uri("http://example.invalid/")
            .body(Buffer::new())
            .expect("request must build");
        let blocked = timeout(Duration::from_millis(50), fetcher.fetch(request)).await;
        assert!(
            blocked.is_err(),
            "http fetch should block while the body holds the permit"
        );
    }
}
