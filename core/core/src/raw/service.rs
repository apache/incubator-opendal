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

use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use crate::raw::*;
use crate::*;

/// Immutable identity and configuration for a storage service.
///
/// `ServiceInfo` excludes runtime resources and composed capabilities so that
/// layers can replace them without mutating the shared service identity.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ServiceInfo {
    scheme: &'static str,
    root: Arc<str>,
    name: Arc<str>,
}

impl Debug for ServiceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceInfo")
            .field("scheme", &self.scheme())
            .field("root", &self.root())
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}

impl ServiceInfo {
    /// Create a new `ServiceInfo`.
    pub fn new(scheme: &'static str, root: impl AsRef<str>, name: impl AsRef<str>) -> Self {
        Self {
            scheme,
            root: Arc::from(root.as_ref()),
            name: Arc::from(name.as_ref()),
        }
    }

    /// Create a new `ServiceInfo` with only scheme.
    pub fn with_scheme(scheme: &'static str) -> Self {
        Self::new(scheme, "", "")
    }

    /// Return a copy of this `ServiceInfo` with a different root.
    pub fn with_root(&self, root: impl AsRef<str>) -> Self {
        Self {
            scheme: self.scheme,
            root: Arc::from(root.as_ref()),
            name: self.name.clone(),
        }
    }

    /// Scheme of the service.
    pub fn scheme(&self) -> &'static str {
        self.scheme
    }

    /// Root of the service. Follows a format like `/path/to/dir/`.
    pub fn root(&self) -> Arc<str> {
        self.root.clone()
    }

    /// Name of the service. This might be empty if the service has no namespace concept.
    ///
    /// For example:
    ///
    /// - `s3` => bucket name
    /// - `azblob` => container name
    /// - `azdfs` => filesystem name
    /// - `azfile` => share name
    pub fn name(&self) -> Arc<str> {
        self.name.clone()
    }
}

/// Foundational trait for storage services.
///
/// Every storage service in OpenDAL implements [`Service`]. Services
/// and layers must implement every operation in this trait and declare their
/// capabilities. This allows callers to detect unsupported operations.
///
/// # Operations
///
/// - An operator normalizes paths before passing them to a `Service`. Relative to
///   the configured `root`:
///   - `/` represents the root.
///   - A path ending with `/` represents a directory.
///   - Any other path represents a file.
/// - Services report their supported operation set through [`Service::capability`].
/// - The [`OperationContext`] carries layer-composed runtime resources for each
///   operation.
pub trait Service: Send + Sync + Debug + Unpin + 'static {
    /// Reader returned by `read`.
    type Reader: oio::Read;
    /// Writer returned by `write`.
    type Writer: oio::Write;
    /// Lister returned by `list`.
    type Lister: oio::List;
    /// Deleter returned by `delete`.
    type Deleter: oio::Delete;
    /// Copier returned by `copy`.
    type Copier: oio::Copy;
    /// Composer returned by `compose`.
    type Composer: oio::Compose;

    /// Return the immutable identity and configuration for this service.
    fn info(&self) -> ServiceInfo;

    /// Return the capability of this service stack.
    ///
    /// Layers may affect a service's capabilities, so callers should use this
    /// value for the current stack instead of assuming the backend's native
    /// capability.
    fn capability(&self) -> Capability;

    /// Invoke the `create` operation on the specified path.
    ///
    /// Requires [`Capability::create_dir`].
    ///
    /// # Behavior
    ///
    /// - `path` is a normalized directory path.
    /// - Creating an existing directory should succeed.
    fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> impl Future<Output = Result<RpCreateDir>> + MaybeSend;

    /// Invoke the `stat` operation on the specified path.
    ///
    /// Requires [`Capability::stat`].
    ///
    /// # Behavior
    ///
    /// - `/` means the service root.
    /// - A path ending with `/` stats a directory.
    /// - Returned metadata must set `mode` and `content_length`.
    fn stat(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpStat,
    ) -> impl Future<Output = Result<RpStat>> + MaybeSend;

    /// Invoke the `read` operation on the specified path.
    ///
    /// Requires [`Capability::read`].
    ///
    /// # Behavior
    ///
    /// - `path` is a normalized file path.
    /// - Range I/O is handled by the returned reader.
    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader>;

    /// Invoke the `write` operation on the specified path.
    ///
    /// Requires [`Capability::write`].
    ///
    /// # Behavior
    ///
    /// - `path` is a normalized file path.
    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer>;

    /// Invoke the `delete` operation.
    ///
    /// Requires [`Capability::delete`].
    ///
    /// # Behavior
    ///
    /// - The returned deleter handles one or more delete requests.
    /// - Deleting a missing path should succeed unless a condition requires a live target.
    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter>;

    /// Invoke the `list` operation on the specified path.
    ///
    /// Requires [`Capability::list`].
    ///
    /// # Behavior
    ///
    /// - `path` is a normalized directory path or prefix.
    /// - Listing a non-existing directory should return an empty stream.
    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister>;

    /// Invoke the `copy` operation on the specified `from` path and `to` path.
    ///
    /// Requires [`Capability::copy`].
    ///
    /// # Behavior
    ///
    /// - `from` and `to` are normalized file paths.
    /// - Copying to an existing file should overwrite and truncate it.
    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier>;

    /// Invoke the `compose` operation for the specified destination path.
    ///
    /// Requires [`Capability::compose`].
    fn compose(
        &self,
        _ctx: &OperationContext,
        _to: &str,
        _args: OpCompose,
    ) -> Result<Self::Composer> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    /// Invoke the `rename` operation on the specified `from` path and `to` path.
    ///
    /// Requires [`Capability::rename`].
    ///
    /// # Behavior
    ///
    /// - `from` and `to` are normalized file paths.
    fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> impl Future<Output = Result<RpRename>> + MaybeSend;

    /// Invoke the `restore` operation on the specified path.
    ///
    /// Requires [`Capability::restore`].
    ///
    /// # Behavior
    ///
    /// - `path` is a normalized file path.
    /// - Without a version, restore reverses the latest recoverable deletion state.
    /// - With a version, restore promotes that historical version to the current version.
    fn restore(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpRestore,
    ) -> impl Future<Output = Result<RpRestore>> + MaybeSend {
        std::future::ready(Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        )))
    }

    /// Invoke the `presign` operation on the specified path.
    ///
    /// Requires [`Capability::presign`] and the matching presign operation
    /// capability.
    fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> impl Future<Output = Result<RpPresign>> + MaybeSend;
}

/// `ServiceDyn` is the dyn version of [`Service`].
pub trait ServiceDyn: Send + Sync + Debug + Unpin + 'static {
    /// Dyn version of [`Service::info`].
    fn info_dyn(&self) -> ServiceInfo;

    /// Dyn version of [`Service::capability`].
    fn capability_dyn(&self) -> Capability;

    /// Dyn version of [`Service::create_dir`].
    fn create_dir_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpCreateDir,
    ) -> BoxedFuture<'a, Result<RpCreateDir>>;

    /// Dyn version of [`Service::stat`].
    fn stat_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpStat,
    ) -> BoxedFuture<'a, Result<RpStat>>;

    /// Dyn version of [`Service::read`].
    fn read_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpRead,
    ) -> Result<oio::Reader>;

    /// Dyn version of [`Service::write`].
    fn write_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpWrite,
    ) -> Result<oio::Writer>;

    /// Dyn version of [`Service::delete`].
    fn delete_dyn<'a>(&'a self, ctx: &'a OperationContext) -> Result<oio::Deleter>;

    /// Dyn version of [`Service::list`].
    fn list_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpList,
    ) -> Result<oio::Lister>;

    /// Dyn version of [`Service::copy`].
    fn copy_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        from: &'a str,
        to: &'a str,
        args: OpCopy,
    ) -> Result<oio::Copier>;

    /// Dyn version of [`Service::compose`].
    fn compose_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        to: &'a str,
        args: OpCompose,
    ) -> Result<oio::Composer>;

    /// Dyn version of [`Service::rename`].
    fn rename_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        from: &'a str,
        to: &'a str,
        args: OpRename,
    ) -> BoxedFuture<'a, Result<RpRename>>;

    /// Dyn version of [`Service::restore`].
    fn restore_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpRestore,
    ) -> BoxedFuture<'a, Result<RpRestore>>;

    /// Dyn version of [`Service::presign`].
    fn presign_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpPresign,
    ) -> BoxedFuture<'a, Result<RpPresign>>;
}

/// Type-erased service handle used by layer composition and operators.
pub type Servicer = Arc<dyn ServiceDyn>;

impl<S: Service + ?Sized> ServiceDyn for S {
    fn info_dyn(&self) -> ServiceInfo {
        self.info()
    }

    fn capability_dyn(&self) -> Capability {
        self.capability()
    }

    fn create_dir_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpCreateDir,
    ) -> BoxedFuture<'a, Result<RpCreateDir>> {
        Box::pin(self.create_dir(ctx, path, args))
    }

    fn stat_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpStat,
    ) -> BoxedFuture<'a, Result<RpStat>> {
        Box::pin(self.stat(ctx, path, args))
    }

    fn read_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpRead,
    ) -> Result<oio::Reader> {
        Ok(Box::new(self.read(ctx, path, args)?) as oio::Reader)
    }

    fn write_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpWrite,
    ) -> Result<oio::Writer> {
        Ok(Box::new(self.write(ctx, path, args)?) as oio::Writer)
    }

    fn delete_dyn<'a>(&'a self, ctx: &'a OperationContext) -> Result<oio::Deleter> {
        Ok(Box::new(self.delete(ctx)?) as oio::Deleter)
    }

    fn list_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpList,
    ) -> Result<oio::Lister> {
        Ok(Box::new(self.list(ctx, path, args)?) as oio::Lister)
    }

    fn copy_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        from: &'a str,
        to: &'a str,
        args: OpCopy,
    ) -> Result<oio::Copier> {
        Ok(Box::new(self.copy(ctx, from, to, args)?) as oio::Copier)
    }

    fn compose_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        to: &'a str,
        args: OpCompose,
    ) -> Result<oio::Composer> {
        Ok(Box::new(self.compose(ctx, to, args)?) as oio::Composer)
    }

    fn rename_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        from: &'a str,
        to: &'a str,
        args: OpRename,
    ) -> BoxedFuture<'a, Result<RpRename>> {
        Box::pin(self.rename(ctx, from, to, args))
    }

    fn restore_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpRestore,
    ) -> BoxedFuture<'a, Result<RpRestore>> {
        Box::pin(self.restore(ctx, path, args))
    }

    fn presign_dyn<'a>(
        &'a self,
        ctx: &'a OperationContext,
        path: &'a str,
        args: OpPresign,
    ) -> BoxedFuture<'a, Result<RpPresign>> {
        Box::pin(self.presign(ctx, path, args))
    }
}

/// Implement `Service` for type-erased services so they use the same API.
impl<T: ServiceDyn + ?Sized> Service for Arc<T> {
    type Reader = oio::Reader;
    type Writer = oio::Writer;
    type Lister = oio::Lister;
    type Deleter = oio::Deleter;
    type Copier = oio::Copier;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.as_ref().info_dyn()
    }

    fn capability(&self) -> Capability {
        self.as_ref().capability_dyn()
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.as_ref().create_dir_dyn(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.as_ref().stat_dyn(ctx, path, args).await
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<oio::Reader> {
        self.as_ref().read_dyn(ctx, path, args)
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<oio::Writer> {
        self.as_ref().write_dyn(ctx, path, args)
    }

    fn delete(&self, ctx: &OperationContext) -> Result<oio::Deleter> {
        self.as_ref().delete_dyn(ctx)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<oio::Lister> {
        self.as_ref().list_dyn(ctx, path, args)
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<oio::Copier> {
        self.as_ref().copy_dyn(ctx, from, to, args)
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<oio::Composer> {
        self.as_ref().compose_dyn(ctx, to, args)
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.as_ref().rename_dyn(ctx, from, to, args).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.as_ref().restore_dyn(ctx, path, args).await
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.as_ref().presign_dyn(ctx, path, args).await
    }
}

/// Dummy implementation of service.
impl Service for () {
    type Reader = ();
    type Writer = ();
    type Lister = ();
    type Deleter = ();
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        ServiceInfo::with_scheme("dummy")
    }

    fn capability(&self) -> Capability {
        Capability::default()
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

    fn read(&self, _: &OperationContext, _: &str, _: OpRead) -> Result<Self::Reader> {
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

    fn delete(&self, _: &OperationContext) -> Result<Self::Deleter> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn list(&self, _: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn copy(&self, _: &OperationContext, _: &str, _: &str, _: OpCopy) -> Result<Self::Copier> {
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

    async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}
