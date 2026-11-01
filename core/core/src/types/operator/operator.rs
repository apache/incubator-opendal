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

use std::future::Future;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;

use crate::operator_futures::*;
use crate::raw::*;
use crate::types::delete::Deleter;
use crate::*;

/// The `Operator` serves as the entry point for all public asynchronous APIs.
///
/// For more details about the `Operator`, refer to the concepts section in the
/// crate documentation.
///
/// ## Immutability
///
/// `Operator` is immutable: methods that change layers, the HTTP transport, or the
/// executor return a new operator. Existing clones and in-flight operations keep
/// using the service stack and operation context they already hold. For example,
/// a lister created before the operator is changed continues to use the previous
/// configuration.
///
/// ### Example
///
/// ```
/// use futures::TryStreamExt;
/// use opendal_core::services::Memory;
/// use opendal_core::{OperationContext, Operator, Result};
///
/// async fn example() -> Result<()> {
///     let op = Operator::new(Memory::default())?;
///     let mut lister = op.lister("/").await?;
///
///     // Replacing the context returns a new operator.
///     let new_op = op.with_context(OperationContext::new());
///     let _ = new_op.list("/").await?;
///
///     // The existing lister still uses the context it captured from `op`.
///     while let Some(entry) = lister.try_next().await? {
///         println!("{}", entry.path());
///     }
///
///     Ok(())
/// }
/// ```
///
/// Internally, an operator keeps base providers and composed dispatch state:
///
/// | Method | Meaning |
/// | --- | --- |
/// | [`Operator::base_service`] | The storage service before user layers are applied. |
/// | [`Operator::base_context`] | The runtime resources before user layers are applied. |
/// | [`Operator::service`] | The storage service stack that receives `read`, `write`, `list`, and other operations. |
/// | [`Operator::context`] | The runtime resources passed with those operations. |
///
/// [`Operator::layer`] appends a layer and replays all layers from the base
/// service and base context. [`Operator::with_context`] replaces the base
/// context and then performs the same replay. This keeps the composed service
/// and context from drifting apart.
///
/// ## Build
///
/// Users can initialize an `Operator` through the following methods:
///
/// - [`Operator::new`]: Creates an operator using a [`services`] builder, such as [`services::Memory`].
/// - [`Operator::from_config`]: Creates an operator using a [`services`] configuration, such as [`services::MemoryConfig`].
/// - [`Operator::from_iter`]: Creates an operator from an iterator of configuration key-value pairs.
///
/// ```
/// # use anyhow::Result;
/// use opendal_core::services::Memory;
/// use opendal_core::Operator;
/// async fn test() -> Result<()> {
///     // Build an `Operator` to start operating the storage.
///     let _: Operator = Operator::new(Memory::default())?;
///
///     Ok(())
/// }
/// ```
///
/// ## Runtime Resources And Layers
///
/// After the operator is built, users can replace runtime resources or add
/// layers on top of it.
///
/// OpenDAL offers various layers for users to choose from. Visit [`layers`] for further details.
///
/// Runtime resources are stored in [`OperationContext`]. HTTP based services
/// receive the composed context for each operation and use its HTTP transport
/// and executor instead of caching those resources in the service built by the
/// builder.
///
/// Layers are replayed from the operator's base service and base context
/// whenever the operator is changed. Cloned operators can therefore add layers
/// or replace providers independently.
///
/// ```
/// use opendal_core::HttpTransporter;
/// use opendal_core::OperationContext;
/// use opendal_core::services::Memory;
/// use opendal_core::Operator;
/// use opendal_core::Result;
///
/// async fn test() -> Result<()> {
///     let op: Operator = Operator::new(Memory::default())?;
///
///     // OpenDAL will replace the default HTTP transport now.
///     let transport = HttpTransporter::default();
///     let op = op.with_context(OperationContext::new().with_http_transport(transport));
///
///     Ok(())
/// }
/// ```
///
/// ## Operate
///
/// After building an operator with the desired configuration and layers, users
/// can start operating on storage.
///
/// The operator is `Send`, `Sync`, and `Clone`. It holds immutable handles, and
/// storage operation APIs only take a `&self` reference, making it safe to share
/// the operator across threads.
///
/// Operator provides a consistent API pattern for data operations. For reading operations, it exposes:
///
/// - [`Operator::read`]: Executes a read operation.
/// - [`Operator::read_with`]: Executes a read operation with additional options using the builder pattern.
/// - [`Operator::read_options`]: Executes a read operation with extra options provided via a [`options::ReadOptions`] struct.
/// - [`Operator::reader`]: Creates a reader for streaming data, allowing for flexible access.
/// - [`Operator::reader_with`]: Creates a reader with advanced options using the builder pattern.
/// - [`Operator::reader_options`]: Creates a reader with extra options provided via a [`options::ReadOptions`] struct.
///
/// The [`Reader`] created by [`Operator`] supports custom read control methods and can be converted
/// into [`futures::AsyncRead`] or [`futures::Stream`] for broader ecosystem compatibility.
///
/// ```no_run
/// use opendal_core::options;
/// use opendal_core::services;
/// use opendal_core::Operator;
/// use opendal_core::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Pick a builder and configure it.
///     let builder = services::Memory::default();
///
///     // Init an operator
///     let op = Operator::new(builder)?;
///
///     // Fetch this file's metadata
///     let meta = op.stat("hello.txt").await?;
///     let length = meta.content_length();
///
///     // Read data from `hello.txt` with options.
///     let bs = op
///         .read_with("hello.txt")
///         .range(0..8 * 1024 * 1024)
///         .chunk(1024 * 1024)
///         .concurrent(4)
///         .await?;
///
///     // The same to:
///     let bs = op
///         .read_options("hello.txt", options::ReadOptions {
///             range: (0..8 * 1024 * 1024).into(),
///             chunk: Some(1024 * 1024),
///             concurrent: 4,
///             ..Default::default()
///         })
///         .await?;
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Operator {
    // Base providers are the bottom slots that layer and resource changes replay from.
    base_srv: Servicer,
    base_ctx: OperationContext,

    // Layers are the replayable program shared by the service, HTTP, and executor planes.
    layers: Arc<Vec<Arc<dyn Layer>>>,

    // Composed dispatch state. `srv` and `ctx` must always come from the same
    // fold over `layers` and the base providers.
    srv: Servicer,
    ctx: OperationContext,
}

impl std::fmt::Debug for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Operator")
            .field("service", &self.srv)
            .field("context", &self.ctx)
            .field("layers", &self.layers)
            .finish_non_exhaustive()
    }
}

/// # Operator basic API.
impl Operator {
    /// Convert a `Servicer` into an operator.
    #[deprecated(since = "0.58.0", note = "use `Operator::from_parts` instead")]
    pub fn from_inner(srv: Servicer) -> Self {
        Self::from_parts(OperationContext::default(), srv)
    }

    /// Build an operator from a pre-composed `ctx` and `service`.
    ///
    /// This is the low-level constructor for callers that already have a
    /// [`Servicer`] and [`OperationContext`]. Most users should use
    /// [`Operator::new`] instead so OpenDAL can install its default layers.
    ///
    /// Pairs with [`Operator::into_parts`]. The operator starts with no layers,
    /// so `ctx` and `service` are also the composed dispatch state.
    pub fn from_parts(ctx: OperationContext, srv: Servicer) -> Self {
        Self {
            base_srv: srv.clone(),
            base_ctx: ctx.clone(),
            layers: Arc::new(Vec::new()),
            srv,
            ctx,
        }
    }

    /// Split the operator into its composed `ctx` and `service` for direct dispatch.
    ///
    /// Base providers and layer replay history are discarded. Reconstructing an
    /// operator with [`Operator::from_parts`] uses the returned values directly
    /// and starts with an empty layer list.
    pub fn into_parts(self) -> (OperationContext, Servicer) {
        (self.ctx, self.srv)
    }

    /// Replay layers over the base providers to compute composed dispatch state.
    ///
    /// `srv` and `ctx` must always come from this single fold, so operation
    /// dispatch cannot mix a service stack with resources from another layer
    /// program.
    fn apply_layers(
        base_srv: &Servicer,
        base_ctx: &OperationContext,
        layers: &[Arc<dyn Layer>],
    ) -> (Servicer, OperationContext) {
        let srv = layers
            .iter()
            .fold(base_srv.clone(), |srv, layer| layer.apply_service(srv));
        let ctx = layers.iter().fold(base_ctx.clone(), |inner, layer| {
            layer.apply_context(srv.clone(), inner)
        });
        (srv, ctx)
    }

    /// Get the storage service before user layers are applied.
    ///
    /// This is useful for code that needs to inspect the original backend. Most
    /// operation code should use [`Operator::service`] instead, because that is
    /// the stack that includes layers.
    pub fn base_service(&self) -> &Servicer {
        &self.base_srv
    }

    /// Get the operation context before user layers are applied.
    ///
    /// This is useful for code that needs to inspect the original runtime
    /// resources. Most operation code should use [`Operator::context`] instead,
    /// because that includes context changes made by layers.
    pub fn base_context(&self) -> &OperationContext {
        &self.base_ctx
    }

    /// Get the storage service stack that receives operations.
    ///
    /// This stack includes the base service plus all user layers applied to the
    /// operator.
    pub fn service(&self) -> &Servicer {
        &self.srv
    }

    /// Get the runtime resources passed with operations.
    ///
    /// This context includes the base context plus all context changes made by
    /// user layers.
    pub fn context(&self) -> &OperationContext {
        &self.ctx
    }

    /// Get information of this operator.
    ///
    /// The effective capability is read from the composed `srv`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// use opendal_core::Operator;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// let info = op.info();
    /// # Ok(())
    /// # }
    /// ```
    pub fn info(&self) -> OperatorInfo {
        OperatorInfo::new(self.srv.info(), self.srv.capability())
    }

    /// Replace the base operation context and rebuild composed state.
    ///
    /// Existing layers are preserved and replayed from the same base service
    /// with the new base context. This is the public API for replacing runtime
    /// resources such as HTTP transport or executor on an operator.
    #[must_use]
    pub fn with_context(self, ctx: OperationContext) -> Self {
        let (srv, composed) = Self::apply_layers(&self.base_srv, &ctx, &self.layers);
        Self {
            base_srv: self.base_srv,
            base_ctx: ctx,
            layers: self.layers,
            srv,
            ctx: composed,
        }
    }

    /// Apply a layer to this operator and rebuild composed state.
    ///
    /// The new layer is appended after existing layers. OpenDAL then replays all
    /// layers from the base service and base context, so the composed service and
    /// composed context are produced by the same ordered layer list.
    #[must_use]
    pub fn layer<L: Layer>(self, layer: L) -> Self {
        let mut layers = Arc::unwrap_or_clone(self.layers);
        layers.push(Arc::new(layer) as Arc<dyn Layer>);
        let layers = Arc::new(layers);
        let (srv, ctx) = Self::apply_layers(&self.base_srv, &self.base_ctx, &layers);
        Self {
            base_srv: self.base_srv,
            base_ctx: self.base_ctx,
            layers,
            srv,
            ctx,
        }
    }
}

/// # Operator async API.
impl Operator {
    /// Check if this operator can work correctly.
    ///
    /// We will send a `list` request to path and return any errors we met.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// use opendal_core::Operator;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.check().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check(&self) -> Result<()> {
        let mut ds = self.lister_with("/").limit(1).await?;

        match ds.next().await {
            Some(Err(e)) if e.kind() != ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Retrieve the metadata for the specified path.
    ///
    /// # Notes
    ///
    /// ## Extra Options
    ///
    /// [`Operator::stat`] is a wrapper around [`Operator::stat_with`] that uses no additional options.
    /// To specify extra options such as `if_match` and `if_none_match`, please use [`Operator::stat_with`] instead.
    ///
    /// # Examples
    ///
    /// ## Check if file exists
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal_core::Operator;
    /// use opendal_core::ErrorKind;
    /// #
    /// # async fn test(op: Operator) -> Result<()> {
    /// if let Err(e) = op.stat("test").await {
    ///     if e.kind() == ErrorKind::NotFound {
    ///         println!("file not exist")
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stat(&self, path: &str) -> Result<Metadata> {
        self.stat_with(path).await
    }

    /// Retrieve the metadata of the specified path with additional options.
    ///
    /// # Options
    ///
    /// Check [`options::StatOptions`] for all available options.
    ///
    /// # Examples
    ///
    /// ## Get metadata while `ETag` matches
    ///
    /// `stat_with` will
    ///
    /// - return `Ok(metadata)` if `ETag` matches
    /// - return `Err(error)` and `error.kind() == ErrorKind::ConditionNotMatch` if file exists but
    ///   `ETag` mismatch
    /// - return `Err(err)` if other errors occur, for example, `NotFound`.
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal_core::Operator;
    /// use opendal_core::ErrorKind;
    /// #
    /// # async fn test(op: Operator) -> Result<()> {
    /// if let Err(e) = op.stat_with("test").if_match("<etag>").await {
    ///     if e.kind() == ErrorKind::ConditionNotMatch {
    ///         println!("file exists, but etag mismatch")
    ///     }
    ///     if e.kind() == ErrorKind::NotFound {
    ///         println!("file not exist")
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn stat_with(&self, path: &str) -> FutureStat<impl Future<Output = Result<Metadata>>> {
        let path = normalize_path(path);
        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            options::StatOptions::default(),
            Self::stat_inner,
        )
    }

    /// Retrieve the metadata of the specified path with additional options.
    ///
    /// # Examples
    ///
    /// ## Get metadata while `ETag` matches
    ///
    /// `stat_with` will
    ///
    /// - return `Ok(metadata)` if `ETag` matches
    /// - return `Err(error)` and `error.kind() == ErrorKind::ConditionNotMatch` if file exists but
    ///   `ETag` mismatch
    /// - return `Err(err)` if other errors occur, for example, `NotFound`.
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal_core::Operator;
    /// use opendal_core::options;
    /// use opendal_core::ErrorKind;
    /// #
    /// # async fn test(op: Operator) -> Result<()> {
    /// let res = op
    ///     .stat_options("test", options::StatOptions {
    ///         if_match: Some("<etag>".to_string()),
    ///         ..Default::default()
    ///     })
    ///     .await;
    /// if let Err(e) = res {
    ///     if e.kind() == ErrorKind::ConditionNotMatch {
    ///         println!("file exists, but etag mismatch")
    ///     }
    ///     if e.kind() == ErrorKind::NotFound {
    ///         println!("file not exist")
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stat_options(&self, path: &str, opts: options::StatOptions) -> Result<Metadata> {
        let path = normalize_path(path);
        Self::stat_inner(self.context().clone(), self.service().clone(), path, opts).await
    }

    #[inline]
    async fn stat_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        opts: options::StatOptions,
    ) -> Result<Metadata> {
        let rp = srv.stat(&ctx, &path, opts.into()).await?;
        Ok(rp.into_metadata())
    }

    /// Check whether this path exists.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal_core::Operator;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let _ = op.exists("test").await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn exists(&self, path: &str) -> Result<bool> {
        let r = self.stat(path).await;
        match r {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Create a directory at the specified path.
    ///
    /// # Notes
    ///
    /// To specify that a path is a directory, you must include a trailing slash (/).
    /// Omitting the trailing slash may cause OpenDAL to return a `NotADirectory` error.
    ///
    /// # Behavior
    ///
    /// - Creating a directory that already exists will succeed.
    /// - Directory creation is always recursive, functioning like `mkdir -p`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.create_dir("path/to/dir/").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_dir(&self, path: &str) -> Result<()> {
        let path = normalize_path(path);

        if !validate_path(&path, EntryMode::DIR) {
            return Err(Error::new(
                ErrorKind::NotADirectory,
                "the path trying to create should end with `/`",
            )
            .with_operation("create_dir")
            .with_context("service", self.srv.info().scheme())
            .with_context("path", &path));
        }

        self.srv
            .create_dir(&self.ctx, &path, OpCreateDir::new())
            .await?;

        Ok(())
    }

    /// Read the entire file into bytes from given path.
    ///
    /// # Notes
    ///
    /// ## Additional Options
    ///
    /// [`Operator::read`] is a simplified method that does not support additional options. To access features like `range` and `if_match`, please use [`Operator::read_with`] or [`Operator::read_options`] instead.
    ///
    /// ## Streaming Read
    ///
    /// This function reads all content into memory at once. For more precise memory management or to read big file lazily, please use [`Operator::reader`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// # use futures::TryStreamExt;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let bs = op.read("path/to/file").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read(&self, path: &str) -> Result<Buffer> {
        self.read_options(path, options::ReadOptions::default())
            .await
    }

    /// Read the entire file into bytes from given path with additional options.
    ///
    /// # Notes
    ///
    /// ## Streaming Read
    ///
    /// This function reads all content into memory at once. For more precise memory management or to read big file lazily, please use [`Operator::reader`].
    ///
    /// # Options
    ///
    /// Visit [`options::ReadOptions`] for all available options.
    ///
    /// # Examples
    ///
    /// Read the first 10 bytes of a file:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let bs = op.read_with("path/to/file").range(0..10).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_with(&self, path: &str) -> FutureRead<impl Future<Output = Result<Buffer>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            options::ReadOptions::default(),
            Self::read_inner,
        )
    }

    /// Read the entire file into bytes from given path with additional options.
    ///
    /// # Notes
    ///
    /// ## Streaming Read
    ///
    /// This function reads all content into memory at once. For more precise memory management or to read big file lazily, please use [`Operator::reader`].
    ///
    /// # Examples
    ///
    /// Read the first 10 bytes of a file:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// use opendal_core::options;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let bs = op
    ///     .read_options("path/to/file", options::ReadOptions {
    ///         range: (0..10).into(),
    ///         ..Default::default()
    ///     })
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read_options(&self, path: &str, opts: options::ReadOptions) -> Result<Buffer> {
        let path = normalize_path(path);
        Self::read_inner(self.context().clone(), self.service().clone(), path, opts).await
    }

    #[inline]
    async fn read_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        opts: options::ReadOptions,
    ) -> Result<Buffer> {
        if !validate_path(&path, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "read path is a directory")
                    .with_operation("read")
                    .with_context("service", srv.info().scheme())
                    .with_context("path", &path),
            );
        }

        let (range, args, opts) = opts.into();
        let reader = srv.read(&ctx, &path, args.clone())?;
        let read_context = ReadContext::new(ctx, srv, path, args, opts, reader);
        let r = Reader::new(read_context);
        let buf = r.read(range).await?;
        Ok(buf)
    }

    /// Create a new reader of given path.
    ///
    /// # Notes
    ///
    /// ## Extra Options
    ///
    /// [`Operator::reader`] is a simplified method without any options. To use additional options such as `concurrent` or `if_match`, please use [`Operator::reader_with`] or [`Operator::reader_options`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// # use futures::TryStreamExt;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let r = op.reader("path/to/file").await?;
    /// // Read the first 10 bytes of the file
    /// let data = r.read(0..10).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn reader(&self, path: &str) -> Result<Reader> {
        self.reader_with(path).await
    }

    /// Create a new reader of given path with additional options.
    ///
    /// # Options
    ///
    /// Visit [`options::ReaderOptions`] for all available options.
    ///
    /// # Examples
    ///
    /// Create a reader with a specific version ID:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let r = op.reader_with("path/to/file").version("version_id").await?;
    /// // Read the first 10 bytes of the file
    /// let data = r.read(0..10).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn reader_with(&self, path: &str) -> FutureReader<impl Future<Output = Result<Reader>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            options::ReaderOptions::default(),
            Self::reader_inner,
        )
    }

    /// Create a new reader of given path with additional options.
    ///
    /// # Examples
    ///
    /// Create a reader with a specific version ID:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// use opendal_core::options;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let r = op
    ///     .reader_options("path/to/file", options::ReaderOptions {
    ///         version: Some("version_id".to_string()),
    ///         ..Default::default()
    ///     })
    ///     .await?;
    /// // Read the first 10 bytes of the file
    /// let data = r.read(0..10).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn reader_options(&self, path: &str, opts: options::ReaderOptions) -> Result<Reader> {
        let path = normalize_path(path);
        Self::reader_inner(self.context().clone(), self.service().clone(), path, opts).await
    }

    /// Allow this unused async since we don't want
    /// to change our public API.
    #[allow(clippy::unused_async)]
    #[inline]
    async fn reader_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        options: options::ReaderOptions,
    ) -> Result<Reader> {
        if !validate_path(&path, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "read path is a directory")
                    .with_operation("Operator::reader")
                    .with_context("service", srv.info().scheme())
                    .with_context("path", path),
            );
        }

        let (args, opts) = options.into();
        let reader = srv.read(&ctx, &path, args.clone())?;
        let read_context = ReadContext::new(ctx, srv, path, args, opts, reader);
        Ok(Reader::new(read_context))
    }

    /// Write all data to the specified path at once.
    ///
    /// # Notes
    ///
    /// Read the [concurrent write guide][crate::docs::performance::concurrent_write]
    /// for more details.
    ///
    /// ## Extra Options
    ///
    /// [`Operator::write`] is a simplified method that does not include additional options.
    /// For advanced features such as `chunk` and `concurrent`, use [`Operator::write_with`] or [`Operator::write_options`] instead.
    ///
    /// ## Streaming Write
    ///
    /// This method executes a single bulk write operation. For more precise memory management
    /// or to write data in a streaming fashion, use [`Operator::writer`] instead.
    ///
    /// ## Multipart Uploads
    ///
    /// OpenDAL offers multipart upload capabilities through the [`Writer`] abstraction,
    /// automatically managing all upload details for you. You can fine-tune the upload process
    /// by adjusting the `chunk` size and the number of `concurrent` operations using [`Operator::writer_with`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// use bytes::Bytes;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.write("path/to/file", vec![0; 4096]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write(&self, path: &str, bs: impl Into<Buffer>) -> Result<Metadata> {
        let bs = bs.into();
        self.write_with(path, bs).await
    }

    /// Write all data to the specified path at once with additional options.
    ///
    /// # Notes
    ///
    /// Read the [concurrent write guide][crate::docs::performance::concurrent_write]
    /// for more details.
    ///
    /// ## Streaming Write
    ///
    /// This method executes a single bulk write operation. For more precise memory management
    /// or to write data in a streaming fashion, use [`Operator::writer`] instead.
    ///
    /// ## Multipart Uploads
    ///
    /// OpenDAL offers multipart upload capabilities through the [`Writer`] abstraction,
    /// automatically managing all upload details for you. You can fine-tune the upload process
    /// by adjusting the `chunk` size and the number of `concurrent` operations using [`Operator::writer_with`].
    ///
    /// # Options
    ///
    /// Visit [`options::WriteOptions`] for all available options.
    ///
    /// # Examples
    ///
    /// Write data to a file only when it does not already exist:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// use bytes::Bytes;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// let _ = op
    ///     .write_with("path/to/file", vec![0; 4096])
    ///     .if_not_exists(true)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_with(
        &self,
        path: &str,
        bs: impl Into<Buffer>,
    ) -> FutureWrite<impl Future<Output = Result<Metadata>>> {
        let path = normalize_path(path);
        let bs = bs.into();

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            (options::WriteOptions::default(), bs),
            |ctx, srv, path, (opts, bs)| Self::write_inner(ctx, srv, path, bs, opts),
        )
    }

    /// Write all data to the specified path at once with additional options.
    ///
    /// # Notes
    ///
    /// Read the [concurrent write guide][crate::docs::performance::concurrent_write]
    /// for more details.
    ///
    /// ## Streaming Write
    ///
    /// This method executes a single bulk write operation. For more precise memory management
    /// or to write data in a streaming fashion, use [`Operator::writer`] instead.
    ///
    /// ## Multipart Uploads
    ///
    /// OpenDAL offers multipart upload capabilities through the [`Writer`] abstraction,
    /// automatically managing all upload details for you. You can fine-tune the upload process
    /// by adjusting the `chunk` size and the number of `concurrent` operations using [`Operator::writer_with`].
    ///
    /// # Examples
    ///
    /// Write data to a file only when it does not already exist:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// use opendal_core::options;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// let _ = op
    ///     .write_options("path/to/file", vec![0; 4096], options::WriteOptions {
    ///         if_not_exists: true,
    ///         ..Default::default()
    ///     })
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write_options(
        &self,
        path: &str,
        bs: impl Into<Buffer>,
        opts: options::WriteOptions,
    ) -> Result<Metadata> {
        let path = normalize_path(path);
        Self::write_inner(
            self.context().clone(),
            self.service().clone(),
            path,
            bs.into(),
            opts,
        )
        .await
    }

    #[inline]
    async fn write_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        bs: Buffer,
        opts: options::WriteOptions,
    ) -> Result<Metadata> {
        if !validate_path(&path, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "write path is a directory")
                    .with_operation("Operator::write")
                    .with_context("service", srv.info().scheme())
                    .with_context("path", &path),
            );
        }

        let mut w = Self::writer_inner(ctx, srv, path, opts).await?;
        w.write(bs).await?;
        w.close().await
    }

    /// Create a new writer of given path.
    ///
    /// # Notes
    ///
    /// ## Writer Features
    ///
    /// The writer provides several powerful capabilities:
    /// - Streaming writes for continuous data transfer
    /// - Automatic multipart upload handling
    /// - Memory-efficient chunk-based writing
    ///
    /// ## Extra Options
    ///
    /// [`Operator::writer`] is a simplified version that does not include additional options.
    /// For advanced features such as `chunk` and `concurrent`, use [`Operator::writer_with`] or [`Operator::writer_options`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// use bytes::Bytes;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut w = op.writer("path/to/file").await?;
    /// w.write(vec![0; 4096]).await?;
    /// w.write(vec![1; 4096]).await?;
    /// w.close().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn writer(&self, path: &str) -> Result<Writer> {
        self.writer_with(path).await
    }

    /// Create a new writer of given path with additional options.
    ///
    /// # Notes
    ///
    /// ## Writer Features
    ///
    /// The writer provides several powerful capabilities:
    /// - Streaming writes for continuous data transfer
    /// - Automatic multipart upload handling
    /// - Memory-efficient chunk-based writing
    ///
    /// ## Chunk Size Handling
    ///
    /// Storage services often have specific requirements for chunk sizes:
    /// - Services like `s3` may return `EntityTooSmall` errors for undersized chunks
    /// - Using small chunks in cloud storage services can lead to increased costs
    ///
    /// OpenDAL automatically determines optimal chunk sizes based on the service's
    /// [Capability](crate::types::Capability). However, you can override this by explicitly
    /// setting the `chunk` parameter.
    ///
    /// Read the [concurrent write guide][crate::docs::performance::concurrent_write]
    /// for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// use bytes::Bytes;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut w = op
    ///     .writer_with("path/to/file")
    ///     .chunk(4 * 1024 * 1024)
    ///     .concurrent(8)
    ///     .await?;
    /// w.write(vec![0; 4096]).await?;
    /// w.write(vec![1; 4096]).await?;
    /// w.close().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn writer_with(&self, path: &str) -> FutureWriter<impl Future<Output = Result<Writer>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            options::WriteOptions::default(),
            Self::writer_inner,
        )
    }

    /// Create a new writer of given path with additional options.
    ///
    /// # Notes
    ///
    /// ## Writer Features
    ///
    /// The writer provides several powerful capabilities:
    /// - Streaming writes for continuous data transfer
    /// - Automatic multipart upload handling
    /// - Memory-efficient chunk-based writing
    ///
    /// ## Chunk Size Handling
    ///
    /// Storage services often have specific requirements for chunk sizes:
    /// - Services like `s3` may return `EntityTooSmall` errors for undersized chunks
    /// - Using small chunks in cloud storage services can lead to increased costs
    ///
    /// OpenDAL automatically determines optimal chunk sizes based on the service's
    /// [Capability](crate::types::Capability). However, you can override this by explicitly
    /// setting the `chunk` parameter.
    ///
    /// Read the [concurrent write guide][crate::docs::performance::concurrent_write]
    /// for more details.
    ///
    /// # Examples
    ///
    /// Write data to a file in 4MiB chunk size and at 8 concurrency:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// # use opendal_core::options;
    /// use bytes::Bytes;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut w = op
    ///     .writer_options(
    ///         "path/to/file",
    ///         options::WriteOptions {
    ///             chunk: Some(4 * 1024 * 1024),
    ///             concurrent: 8,
    ///           ..Default::default()
    ///         },
    ///     )
    ///     .await?;
    /// w.write(vec![0; 4096]).await?;
    /// w.write(vec![1; 4096]).await?;
    /// w.close().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn writer_options(&self, path: &str, opts: options::WriteOptions) -> Result<Writer> {
        let path = normalize_path(path);
        Self::writer_inner(self.context().clone(), self.service().clone(), path, opts).await
    }

    // Keep this async so it can be used as an `OperatorFuture` factory.
    #[allow(clippy::unused_async)]
    #[inline]
    async fn writer_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        opts: options::WriteOptions,
    ) -> Result<Writer> {
        if !validate_path(&path, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "write path is a directory")
                    .with_operation("Operator::writer")
                    .with_context("service", srv.info().scheme())
                    .with_context("path", &path),
            );
        }

        let (args, opts) = OpWrite::from_options(&srv.capability(), opts)
            .map_err(|err| err.with_context("service", srv.info().scheme()))?;
        let write_context = WriteContext::new(ctx, srv, path, args, opts);
        let w = Writer::new(write_context)?;
        Ok(w)
    }

    /// Compose complete source objects into `to` in iteration order.
    ///
    /// The input must be non-empty, and `to` must not be one of the inputs.
    /// Composition preserves every source object. It does not read source bytes
    /// through the client when the service supports native composition.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let _metadata = op
    ///     .compose(["parts/0", "parts/1", "parts/2"], "result")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn compose<I, D>(&self, inputs: I, to: &str) -> Result<Metadata>
    where
        I: IntoIterator<Item = D>,
        D: IntoComposeInput,
    {
        self.compose_with(inputs, to).await
    }

    /// Compose complete source objects into `to` with additional options.
    ///
    /// Visit [`options::ComposeOptions`] for all available options.
    pub fn compose_with<I, D>(
        &self,
        inputs: I,
        to: &str,
    ) -> FutureCompose<impl Future<Output = Result<Metadata>>>
    where
        I: IntoIterator<Item = D>,
        D: IntoComposeInput,
    {
        let to = normalize_path(to);
        let inputs = inputs
            .into_iter()
            .map(IntoComposeInput::into_compose_input)
            .collect();

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            to,
            (options::ComposeOptions::default(), inputs),
            Self::compose_inner,
        )
    }

    /// Compose complete source objects into `to` with explicit options.
    ///
    /// Visit [`options::ComposeOptions`] for all available options.
    pub async fn compose_options<I, D>(
        &self,
        inputs: I,
        to: &str,
        opts: options::ComposeOptions,
    ) -> Result<Metadata>
    where
        I: IntoIterator<Item = D>,
        D: IntoComposeInput,
    {
        let to = normalize_path(to);
        let inputs = inputs
            .into_iter()
            .map(IntoComposeInput::into_compose_input)
            .collect();
        Self::compose_inner(
            self.context().clone(),
            self.service().clone(),
            to,
            (opts, inputs),
        )
        .await
    }

    async fn compose_inner(
        ctx: OperationContext,
        srv: Servicer,
        to: String,
        (opts, inputs): (
            options::ComposeOptions,
            Vec<(String, options::ComposeSourceOptions)>,
        ),
    ) -> Result<Metadata> {
        let mut composer = Self::composer_inner(ctx, srv, to, opts).await?;
        for (path, options) in inputs {
            composer.compose_options(&path, options).await?;
        }
        composer.close().await
    }

    /// Create a [`Composer`] that accepts ordered source objects for `to`.
    pub async fn composer(&self, to: &str) -> Result<Composer> {
        self.composer_with(to).await
    }

    /// Create a [`Composer`] for `to` with additional options.
    ///
    /// Visit [`options::ComposeOptions`] for all available options.
    pub fn composer_with(
        &self,
        to: &str,
    ) -> FutureComposer<impl Future<Output = Result<Composer>>> {
        let to = normalize_path(to);
        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            to,
            options::ComposeOptions::default(),
            Self::composer_inner,
        )
    }

    /// Create a [`Composer`] for `to` with explicit options.
    ///
    /// Visit [`options::ComposeOptions`] for all available options.
    pub async fn composer_options(
        &self,
        to: &str,
        opts: options::ComposeOptions,
    ) -> Result<Composer> {
        let to = normalize_path(to);
        Self::composer_inner(self.context().clone(), self.service().clone(), to, opts).await
    }

    // Keep this async so it can be used as an `OperatorFuture` factory.
    #[allow(clippy::unused_async)]
    async fn composer_inner(
        ctx: OperationContext,
        srv: Servicer,
        to: String,
        opts: options::ComposeOptions,
    ) -> Result<Composer> {
        if !validate_path(&to, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "destination path is a directory")
                    .with_operation(Operation::Compose.into_static())
                    .with_context("service", srv.info().scheme())
                    .with_context("to", to),
            );
        }

        let args = OpCompose::from_options(&srv.capability(), opts)
            .map_err(|err| err.with_context("service", srv.info().scheme()))?;
        Composer::create(ctx, srv, to, args)
    }

    /// Copy a file from `from` to `to`.
    ///
    /// # Notes
    ///
    /// - `from` and `to` must be a file.
    /// - `to` will be overwritten if it exists.
    /// - If `from` and `to` are the same,  an `IsSameFile` error will occur.
    /// - `copy` is idempotent. For same `from` and `to` input, the result will be the same.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.copy("path/to/file", "path/to/file2").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn copy(&self, from: &str, to: &str) -> Result<Metadata> {
        self.copy_options(from, to, options::CopyOptions::default())
            .await
    }

    /// Create a copier from `from` to `to`.
    pub async fn copier(&self, from: &str, to: &str) -> Result<Copier> {
        self.copier_options(from, to, options::CopyOptions::default())
            .await
    }

    /// Copy a file from `from` to `to` with additional options.
    ///
    /// # Notes
    ///
    /// - `from` and `to` must be a file.
    /// - If `from` and `to` are the same and `source_version` is not set,
    ///   an `IsSameFile` error will occur.
    /// - `copy` is idempotent. For same `from` and `to` input, the result will be the same.
    ///
    /// # Options
    ///
    /// Visit [`options::CopyOptions`] for all available options.
    ///
    /// # Examples
    ///
    /// Copy a file only if the destination doesn't exist:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.copy_with("path/to/file", "path/to/file2")
    ///     .if_not_exists(true)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn copy_with(
        &self,
        from: &str,
        to: &str,
    ) -> FutureCopy<impl Future<Output = Result<Metadata>>> {
        let from = normalize_path(from);
        let to = normalize_path(to);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            from,
            (options::CopyOptions::default(), to),
            Self::copy_inner,
        )
    }

    /// Create a copier from `from` to `to` with additional options.
    pub fn copier_with(
        &self,
        from: &str,
        to: &str,
    ) -> FutureCopier<impl Future<Output = Result<Copier>>> {
        let from = normalize_path(from);
        let to = normalize_path(to);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            from,
            (options::CopyOptions::default(), to),
            Self::copier_inner,
        )
    }

    /// Copy a file from `from` to `to` with additional options.
    ///
    /// # Notes
    ///
    /// - `from` and `to` must be a file.
    /// - If `from` and `to` are the same and `source_version` is not set,
    ///   an `IsSameFile` error will occur.
    /// - `copy` is idempotent. For same `from` and `to` input, the result will be the same.
    ///
    /// # Options
    ///
    /// Check [`options::CopyOptions`] for all available options.
    ///
    /// # Examples
    ///
    /// Copy a file only if the destination doesn't exist:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// # use opendal_core::options::CopyOptions;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut opts = CopyOptions::default();
    /// opts.if_not_exists = true;
    /// op.copy_options("path/to/file", "path/to/file2", opts).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn copy_options(
        &self,
        from: &str,
        to: &str,
        opts: impl Into<options::CopyOptions>,
    ) -> Result<Metadata> {
        let from = normalize_path(from);
        let to = normalize_path(to);
        let opts = opts.into();

        Self::copy_inner(
            self.context().clone(),
            self.service().clone(),
            from,
            (opts, to),
        )
        .await
    }

    /// Create a copier from `from` to `to` with additional options.
    pub async fn copier_options(
        &self,
        from: &str,
        to: &str,
        opts: impl Into<options::CopyOptions>,
    ) -> Result<Copier> {
        let from = normalize_path(from);
        let to = normalize_path(to);
        let opts = opts.into();

        Self::copier_inner(
            self.context().clone(),
            self.service().clone(),
            from,
            (opts, to),
        )
        .await
    }

    async fn copy_inner(
        ctx: OperationContext,
        srv: Servicer,
        from: String,
        (opts, to): (options::CopyOptions, String),
    ) -> Result<Metadata> {
        if !validate_path(&from, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "from path is a directory")
                    .with_operation("Operator::copy")
                    .with_context("service", srv.info().scheme())
                    .with_context("from", from),
            );
        }

        if !validate_path(&to, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "to path is a directory")
                    .with_operation("Operator::copy")
                    .with_context("service", srv.info().scheme())
                    .with_context("to", to),
            );
        }

        if from == to && opts.source_version.is_none() {
            return Err(
                Error::new(ErrorKind::IsSameFile, "from and to paths are same")
                    .with_operation("Operator::copy")
                    .with_context("service", srv.info().scheme())
                    .with_context("from", &from)
                    .with_context("to", &to),
            );
        }

        let mut copier = Self::copier_inner(ctx, srv, from, (opts, to)).await?;
        match copier.close().await {
            Ok(meta) => Ok(meta),
            Err(err) => {
                let _ = copier.abort().await;
                Err(err)
            }
        }
    }

    // Keep this async so it can be used as an `OperatorFuture` factory.
    #[allow(clippy::unused_async)]
    async fn copier_inner(
        ctx: OperationContext,
        srv: Servicer,
        from: String,
        (opts, to): (options::CopyOptions, String),
    ) -> Result<Copier> {
        if !validate_path(&from, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "from path is a directory")
                    .with_operation("Operator::copier")
                    .with_context("service", srv.info().scheme())
                    .with_context("from", from),
            );
        }

        if !validate_path(&to, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "to path is a directory")
                    .with_operation("Operator::copier")
                    .with_context("service", srv.info().scheme())
                    .with_context("to", to),
            );
        }

        if from == to && opts.source_version.is_none() {
            return Err(
                Error::new(ErrorKind::IsSameFile, "from and to paths are same")
                    .with_operation("Operator::copier")
                    .with_context("service", srv.info().scheme())
                    .with_context("from", &from)
                    .with_context("to", &to),
            );
        }

        let args = OpCopy::from_options(&srv.capability(), opts)
            .map_err(|err| err.with_context("service", srv.info().scheme()))?;
        Copier::create(ctx, srv, &from, &to, args)
    }

    /// Rename a file from `from` to `to`.
    ///
    /// # Notes
    ///
    /// - `from` and `to` must be a file.
    /// - `to` will be overwritten if it exists.
    /// - If `from` and `to` are the same, an `IsSameFile` error will occur.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    ///
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.rename("path/to/file", "path/to/file2").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.rename_options(from, to, options::RenameOptions::default())
            .await
    }

    /// Rename a file from `from` to `to` with additional options.
    ///
    /// # Notes
    ///
    /// - `from` and `to` must be a file.
    /// - If `from` and `to` are the same, an `IsSameFile` error will occur.
    ///
    /// # Options
    ///
    /// Visit [`options::RenameOptions`] for all available options.
    ///
    /// # Examples
    ///
    /// ```
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn rename_with_options(op: Operator) -> Result<()> {
    ///     op.rename_with("path/to/file", "path/to/file2")
    ///         .if_not_exists(true)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn rename_with(
        &self,
        from: &str,
        to: &str,
    ) -> FutureRename<impl Future<Output = Result<()>>> {
        let from = normalize_path(from);
        let to = normalize_path(to);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            from,
            (options::RenameOptions::default(), to),
            Self::rename_inner,
        )
    }

    /// Rename a file from `from` to `to` with additional options.
    ///
    /// # Options
    ///
    /// Visit [`options::RenameOptions`] for all available options.
    ///
    /// # Examples
    ///
    /// ```
    /// use opendal_core::options::RenameOptions;
    /// use opendal_core::Operator;
    /// use opendal_core::Result;
    ///
    /// async fn rename_with_test(op: Operator) -> Result<()> {
    ///     let mut opts = RenameOptions::default();
    ///     opts.if_not_exists = true;
    ///     op.rename_options("path/to/file", "path/to/file2", opts)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn rename_options(
        &self,
        from: &str,
        to: &str,
        opts: impl Into<options::RenameOptions>,
    ) -> Result<()> {
        let from = normalize_path(from);
        let to = normalize_path(to);
        let opts = opts.into();

        Self::rename_inner(
            self.context().clone(),
            self.service().clone(),
            from,
            (opts, to),
        )
        .await
    }

    async fn rename_inner(
        ctx: OperationContext,
        srv: Servicer,
        from: String,
        (opts, to): (options::RenameOptions, String),
    ) -> Result<()> {
        if !validate_path(&from, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "from path is a directory")
                    .with_operation(Operation::Rename)
                    .with_context("service", srv.info().scheme())
                    .with_context("from", from),
            );
        }

        if !validate_path(&to, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "to path is a directory")
                    .with_operation(Operation::Rename)
                    .with_context("service", srv.info().scheme())
                    .with_context("to", to),
            );
        }

        if from == to {
            return Err(
                Error::new(ErrorKind::IsSameFile, "from and to paths are same")
                    .with_operation(Operation::Rename)
                    .with_context("service", srv.info().scheme())
                    .with_context("from", from)
                    .with_context("to", to),
            );
        }

        srv.rename(&ctx, &from, &to, opts.into()).await?;

        Ok(())
    }

    /// Restore the given path from its latest deleted state.
    ///
    /// Calling `restore` on an already-live path succeeds. Calling it when no
    /// live object or recoverable deletion state exists returns
    /// [`ErrorKind::NotFound`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.restore("path/to/file").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn restore(&self, path: &str) -> Result<()> {
        self.restore_with(path).await
    }

    /// Restore the given path with additional options.
    ///
    /// # Examples
    ///
    /// Restore a specific version only if the path has not been recreated:
    ///
    /// ```
    /// # use opendal_core::Operator;
    /// # use opendal_core::Result;
    /// # async fn test(op: Operator, version: &str) -> Result<()> {
    /// op.restore_with("path/to/file")
    ///     .version(version)
    ///     .if_not_exists(true)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn restore_with(&self, path: &str) -> FutureRestore<impl Future<Output = Result<()>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            options::RestoreOptions::default(),
            Self::restore_inner,
        )
    }

    /// Restore the given path with additional options.
    pub async fn restore_options(
        &self,
        path: &str,
        opts: impl Into<options::RestoreOptions>,
    ) -> Result<()> {
        let path = normalize_path(path);
        let opts = opts.into();

        Self::restore_inner(self.context().clone(), self.service().clone(), path, opts).await
    }

    async fn restore_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        opts: options::RestoreOptions,
    ) -> Result<()> {
        if !validate_path(&path, EntryMode::FILE) {
            return Err(Error::new(ErrorKind::IsADirectory, "path is a directory")
                .with_operation(Operation::Restore)
                .with_context("service", srv.info().scheme())
                .with_context("path", path));
        }

        if opts.if_not_exists && opts.version.is_none() {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "if_not_exists requires a restore version",
            )
            .with_operation(Operation::Restore)
            .with_context("service", srv.info().scheme())
            .with_context("path", path));
        }

        srv.restore(&ctx, &path, opts.into()).await?;
        Ok(())
    }

    /// Delete the given path.
    ///
    /// # Notes
    ///
    /// - Deleting a file that does not exist is a successful no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.delete("test").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(&self, path: &str) -> Result<()> {
        self.delete_with(path).await
    }

    /// Delete the given path with additional options.
    ///
    /// # Notes
    ///
    /// - Deleting a file that does not exist is a successful no-op, unless a
    ///   condition such as `if_match` fails first.
    ///
    /// # Options
    ///
    /// Visit [`options::DeleteOptions`] for all available options and the
    /// conditional delete contract.
    ///
    /// # Examples
    ///
    /// Delete a specific version of a file:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    ///
    /// # async fn test(op: Operator, version: &str) -> Result<()> {
    /// op.delete_with("path/to/file").version(version).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_with(&self, path: &str) -> FutureDelete<impl Future<Output = Result<()>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            options::DeleteOptions::default(),
            Self::delete_inner,
        )
    }

    /// Delete the given path with additional options.
    ///
    /// # Notes
    ///
    /// - Deleting a file that does not exist is a successful no-op, unless a
    ///   condition such as `if_match` fails first. Visit
    ///   [`options::DeleteOptions`] for the conditional delete contract.
    ///
    /// # Examples
    ///
    /// Delete a specific version of a file:
    ///
    /// ```
    /// # use opendal_core::Result;
    /// # use opendal_core::Operator;
    /// use opendal_core::options;
    ///
    /// # async fn test(op: Operator, version: &str) -> Result<()> {
    /// op.delete_options("path/to/file", options::DeleteOptions {
    ///     version: Some(version.to_string()),
    ///     ..Default::default()
    /// })
    /// .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_options(&self, path: &str, opts: options::DeleteOptions) -> Result<()> {
        let path = normalize_path(path);
        Self::delete_inner(self.context().clone(), self.service().clone(), path, opts).await
    }

    async fn delete_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        opts: options::DeleteOptions,
    ) -> Result<()> {
        let mut deleter = Deleter::create(ctx, srv)?;
        deleter.delete((path, opts)).await?;
        deleter.close().await?;
        Ok(())
    }

    /// Delete an infallible iterator of paths.
    ///
    /// Also see:
    ///
    /// - [`Operator::delete_try_iter`]: delete a fallible iterator of paths.
    /// - [`Operator::delete_stream`]: delete an infallible stream of paths.
    /// - [`Operator::delete_try_stream`]: delete a fallible stream of paths.
    pub async fn delete_iter<I, D>(&self, iter: I) -> Result<()>
    where
        I: IntoIterator<Item = D>,
        D: IntoDeleteInput,
    {
        let mut deleter = self.deleter().await?;
        deleter.delete_iter(iter).await?;
        deleter.close().await?;
        Ok(())
    }

    /// Delete a fallible iterator of paths.
    ///
    /// Also see:
    ///
    /// - [`Operator::delete_iter`]: delete an infallible iterator of paths.
    /// - [`Operator::delete_stream`]: delete an infallible stream of paths.
    /// - [`Operator::delete_try_stream`]: delete a fallible stream of paths.
    pub async fn delete_try_iter<I, D>(&self, try_iter: I) -> Result<()>
    where
        I: IntoIterator<Item = Result<D>>,
        D: IntoDeleteInput,
    {
        let mut deleter = self.deleter().await?;
        deleter.delete_try_iter(try_iter).await?;
        deleter.close().await?;
        Ok(())
    }

    /// Delete an infallible stream of paths.
    ///
    /// Also see:
    ///
    /// - [`Operator::delete_iter`]: delete an infallible iterator of paths.
    /// - [`Operator::delete_try_iter`]: delete a fallible iterator of paths.
    /// - [`Operator::delete_try_stream`]: delete a fallible stream of paths.
    pub async fn delete_stream<S, D>(&self, stream: S) -> Result<()>
    where
        S: Stream<Item = D>,
        D: IntoDeleteInput,
    {
        let mut deleter = self.deleter().await?;
        deleter.delete_stream(stream).await?;
        deleter.close().await?;
        Ok(())
    }

    /// Delete a fallible stream of paths.
    ///
    /// Also see:
    ///
    /// - [`Operator::delete_iter`]: delete an infallible iterator of paths.
    /// - [`Operator::delete_try_iter`]: delete a fallible iterator of paths.
    /// - [`Operator::delete_stream`]: delete an infallible stream of paths.
    pub async fn delete_try_stream<S, D>(&self, try_stream: S) -> Result<()>
    where
        S: Stream<Item = Result<D>>,
        D: IntoDeleteInput,
    {
        let mut deleter = self.deleter().await?;
        deleter.delete_try_stream(try_stream).await?;
        deleter.close().await?;
        Ok(())
    }

    /// Create a [`Deleter`] to continuously remove content from storage.
    ///
    /// It leverages batch deletion capabilities provided by storage services for efficient removal.
    ///
    /// Users can have more control over the deletion process by using [`Deleter`] directly.
    // Keep this async to preserve the public API.
    #[allow(clippy::unused_async)]
    pub async fn deleter(&self) -> Result<Deleter> {
        Deleter::create(self.context().clone(), self.service().clone())
    }

    /// Remove the path and all nested dirs and files recursively.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated since v0.55.0. Use [`Operator::delete_with`] with
    /// `recursive(true)` instead.
    ///
    /// ## Migration Example
    ///
    /// Instead of:
    /// ```ignore
    /// op.remove_all("path/to/dir").await?;
    /// ```
    ///
    /// Use:
    /// ```ignore
    /// op.delete_with("path/to/dir").recursive(true).await?;
    /// ```
    ///
    /// # Notes
    ///
    /// If underlying services support delete in batch, we will use batch
    /// delete instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal_core::Operator;
    /// #
    /// # async fn test(op: Operator) -> Result<()> {
    /// op.remove_all("path/to/dir").await?;
    /// # Ok(())
    /// # }
    /// ```
    #[deprecated(
        since = "0.55.0",
        note = "Use `delete_with` with `recursive(true)` instead"
    )]
    pub async fn remove_all(&self, path: &str) -> Result<()> {
        self.delete_with(path).recursive(true).await
    }

    /// List entries whose paths start with the given prefix `path`.
    ///
    /// # Semantics
    ///
    /// - Listing is **prefix-based**. It does not require the parent directory to exist.
    /// - If `path` itself exists (file or dir), it will be returned as an entry in addition to any prefixed children.
    /// - If `path` is absent but deeper objects exist (e.g. `path/child`), the list succeeds and returns those prefixed entries instead of an error.
    ///
    /// ## Streaming List
    ///
    /// This function materializes the entire list into memory. For large listings, prefer [`Operator::lister`] to stream entries.
    ///
    /// # Examples
    ///
    /// This example will list all entries under the dir `path/to/dir/`.
    ///
    /// ```
    /// # use anyhow::Result;
    /// use opendal_core::EntryMode;
    /// use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut entries = op.list("path/to/dir/").await?;
    /// for entry in entries {
    ///     match entry.metadata().mode() {
    ///         EntryMode::FILE => {
    ///             println!("Handling file")
    ///         }
    ///         EntryMode::DIR => {
    ///             println!("Handling dir {}", entry.path())
    ///         }
    ///         EntryMode::Unknown => continue,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list(&self, path: &str) -> Result<Vec<Entry>> {
        self.list_with(path).await
    }

    /// List entries whose paths start with the given prefix `path` with additional options.
    ///
    /// # Semantics
    ///
    /// Inherits the prefix semantics described in [`Operator::list`]: returns `path` itself if it exists and tolerates missing parents when prefixed objects exist.
    ///
    /// # Notes
    ///
    /// ## Streaming List
    ///
    /// This function materializes the entire list into memory. For large listings, prefer [`Operator::lister`] to stream entries.
    ///
    /// ## Options
    ///
    /// See [`options::ListOptions`] for the full set. Common knobs:
    /// - Traversal: `recursive` (default `false`) toggles depth-first listing under the prefix.
    /// - Pagination: `limit` and `start_after` tune page size and resume positions (backend dependent).
    /// - Versioning: `versions` / `deleted` ask versioned backends to return extra entries.
    ///
    /// # Examples
    ///
    /// This example will list all entries recursively under the prefix `path/to/prefix`.
    ///
    /// ```
    /// # use anyhow::Result;
    /// use opendal_core::EntryMode;
    /// use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut entries = op.list_with("path/to/prefix").recursive(true).await?;
    /// for entry in entries {
    ///     match entry.metadata().mode() {
    ///         EntryMode::FILE => {
    ///             println!("Handling file")
    ///         }
    ///         EntryMode::DIR => {
    ///             println!("Handling dir like start a new list via meta.path()")
    ///         }
    ///         EntryMode::Unknown => continue,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_with(&self, path: &str) -> FutureList<impl Future<Output = Result<Vec<Entry>>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            options::ListOptions::default(),
            Self::list_inner,
        )
    }

    /// List entries whose paths start with the given prefix `path` using explicit options.
    ///
    /// # Semantics
    ///
    /// Same prefix behavior as [`Operator::list`]: returns `path` itself if present and tolerates missing parents when prefixed objects exist.
    ///
    /// # Options
    ///
    /// Accepts [`options::ListOptions`] (see field docs for meaning).
    ///
    /// ## Streaming List
    ///
    /// Materializes the entire list; use [`Operator::lister`] to stream large result sets.
    ///
    /// # Examples
    ///
    /// This example will list all entries recursively under the prefix `path/to/prefix`.
    ///
    /// ```
    /// # use anyhow::Result;
    /// use opendal_core::options;
    /// use opendal_core::EntryMode;
    /// use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut entries = op
    ///     .list_options("path/to/prefix", options::ListOptions {
    ///         recursive: true,
    ///         ..Default::default()
    ///     })
    ///     .await?;
    /// for entry in entries {
    ///     match entry.metadata().mode() {
    ///         EntryMode::FILE => {
    ///             println!("Handling file")
    ///         }
    ///         EntryMode::DIR => {
    ///             println!("Handling dir like start a new list via meta.path()")
    ///         }
    ///         EntryMode::Unknown => continue,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_options(&self, path: &str, opts: options::ListOptions) -> Result<Vec<Entry>> {
        let path = normalize_path(path);
        Self::list_inner(self.context().clone(), self.service().clone(), path, opts).await
    }

    #[inline]
    async fn list_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        opts: options::ListOptions,
    ) -> Result<Vec<Entry>> {
        let args = opts.into();
        let lister = Lister::create(ctx, srv, &path, args)?;
        lister.try_collect().await
    }

    /// Create a streaming lister for entries whose paths start with the given prefix `path`.
    ///
    /// # Semantics
    ///
    /// Shares the same prefix semantics as [`Operator::list`]: the parent directory is not required to exist; the entry for `path` is yielded if present; missing parents with deeper objects are accepted.
    ///
    /// # Options
    ///
    /// Takes the same [`options::ListOptions`] as [`list_with`](Operator::list_with): traversal (`recursive`), pagination (`limit`, `start_after`), and versioning (`versions`, `deleted`).
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// use futures::TryStreamExt;
    /// use opendal_core::EntryMode;
    /// use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut ds = op.lister("path/to/dir/").await?;
    /// while let Some(mut de) = ds.try_next().await? {
    ///     match de.metadata().mode() {
    ///         EntryMode::FILE => {
    ///             println!("Handling file")
    ///         }
    ///         EntryMode::DIR => {
    ///             println!("Handling dir like start a new list via meta.path()")
    ///         }
    ///         EntryMode::Unknown => continue,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn lister(&self, path: &str) -> Result<Lister> {
        self.lister_with(path).await
    }

    /// Create a new lister to list entries that start with the given prefix `path` using additional options.
    ///
    /// # Options
    ///
    /// Same as [`lister_with`](Operator::lister_with); see [`options::ListOptions`] for traversal, pagination, and versioning knobs.
    ///
    /// # Examples
    ///
    /// ## List all files recursively
    ///
    /// ```
    /// # use anyhow::Result;
    /// use futures::TryStreamExt;
    /// use opendal_core::EntryMode;
    /// use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut lister = op.lister_with("path/to/dir/").recursive(true).await?;
    /// while let Some(mut entry) = lister.try_next().await? {
    ///     match entry.metadata().mode() {
    ///         EntryMode::FILE => {
    ///             println!("Handling file {}", entry.path())
    ///         }
    ///         EntryMode::DIR => {
    ///             println!("Handling dir {}", entry.path())
    ///         }
    ///         EntryMode::Unknown => continue,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn lister_with(&self, path: &str) -> FutureLister<impl Future<Output = Result<Lister>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            options::ListOptions::default(),
            Self::lister_inner,
        )
    }

    /// Create a new lister to list entries that start with the given prefix `path` using additional options.
    ///
    /// # Semantics
    ///
    /// Inherits the prefix behavior of [`Operator::lister_with`].
    ///
    /// # Options
    ///
    /// Uses [`options::ListOptions`] to control traversal, pagination, and versioning.
    ///
    /// # Examples
    ///
    /// ## List all files recursively
    ///
    /// ```
    /// # use anyhow::Result;
    /// use futures::TryStreamExt;
    /// use opendal_core::options;
    /// use opendal_core::EntryMode;
    /// use opendal_core::Operator;
    /// # async fn test(op: Operator) -> Result<()> {
    /// let mut lister = op
    ///     .lister_options("path/to/dir/", options::ListOptions {
    ///         recursive: true,
    ///         ..Default::default()
    ///     })
    ///     .await?;
    /// while let Some(mut entry) = lister.try_next().await? {
    ///     match entry.metadata().mode() {
    ///         EntryMode::FILE => {
    ///             println!("Handling file {}", entry.path())
    ///         }
    ///         EntryMode::DIR => {
    ///             println!("Handling dir {}", entry.path())
    ///         }
    ///         EntryMode::Unknown => continue,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn lister_options(&self, path: &str, opts: options::ListOptions) -> Result<Lister> {
        let path = normalize_path(path);
        Self::lister_inner(self.context().clone(), self.service().clone(), path, opts).await
    }

    // Keep this async so it can be used as an `OperatorFuture` factory.
    #[allow(clippy::unused_async)]
    #[inline]
    async fn lister_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        opts: options::ListOptions,
    ) -> Result<Lister> {
        let args = opts.into();
        Lister::create(ctx, srv, &path, args)
    }
}

/// Operator presign API.
impl Operator {
    /// Presign an operation for stat(head).
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal_core::Operator;
    /// use std::time::Duration;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op.presign_stat("test",Duration::from_secs(3600)).await?;
    ///     let req = http::Request::builder()
    ///         .method(signed_req.method())
    ///         .uri(signed_req.uri())
    ///         .body(())?;
    ///
    /// #    Ok(())
    /// # }
    /// ```
    pub async fn presign_stat(&self, path: &str, expire: Duration) -> Result<PresignedRequest> {
        self.presign_stat_with(path, expire).await
    }

    /// Presign an operation for stat(head).
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal_core::Operator;
    /// use std::time::Duration;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op.presign_stat_with("test",Duration::from_secs(3600)).override_content_disposition("attachment; filename=\"othertext.txt\"").await?;
    /// #    Ok(())
    /// # }
    /// ```
    pub fn presign_stat_with(
        &self,
        path: &str,
        expire: Duration,
    ) -> FuturePresignStat<impl Future<Output = Result<PresignedRequest>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            (options::StatOptions::default(), expire),
            Self::presign_stat_inner,
        )
    }

    /// Presign an operation for stat(head) with additional options.
    ///
    /// # Options
    ///
    /// Visit [`options::StatOptions`] for all available options.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use opendal_core::Operator;
    /// use opendal_core::options;
    /// use std::time::Duration;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op.presign_stat_options(
    ///         "test",
    ///         Duration::from_secs(3600),
    ///         options::StatOptions {
    ///             if_match: Some("<etag>".to_string()),
    ///             ..Default::default()
    ///         }
    ///     ).await?;
    ///     let req = http::Request::builder()
    ///         .method(signed_req.method())
    ///         .uri(signed_req.uri())
    ///         .body(())?;
    ///
    /// #    Ok(())
    /// # }
    /// ```
    pub async fn presign_stat_options(
        &self,
        path: &str,
        expire: Duration,
        opts: options::StatOptions,
    ) -> Result<PresignedRequest> {
        let path = normalize_path(path);
        Self::presign_stat_inner(
            self.context().clone(),
            self.service().clone(),
            path,
            (opts, expire),
        )
        .await
    }

    #[inline]
    async fn presign_stat_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        (opts, expire): (options::StatOptions, Duration),
    ) -> Result<PresignedRequest> {
        let op = OpPresign::new(OpStat::from(opts), expire);
        let rp = srv.presign(&ctx, &path, op).await?;
        Ok(rp.into_presigned_request())
    }

    /// Presign an operation for read.
    ///
    /// # Notes
    ///
    /// ## Extra Options
    ///
    /// `presign_read` is a wrapper of [`Self::presign_read_with`] without any options. To use
    /// extra options like `override_content_disposition`, please use [`Self::presign_read_with`] or
    /// [`Self::presign_read_options] instead.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal_core::Operator;
    /// use std::time::Duration;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op.presign_read("test.txt", Duration::from_secs(3600)).await?;
    /// #    Ok(())
    /// # }
    /// ```
    ///
    /// - `signed_req.method()`: `GET`
    /// - `signed_req.uri()`: `https://s3.amazonaws.com/examplebucket/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=access_key_id/20130721/us-east-1/s3/aws4_request&X-Amz-Date=20130721T201207Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host&X-Amz-Signature=<signature-value>`
    /// - `signed_req.headers()`: `{ "host": "s3.amazonaws.com" }`
    ///
    /// We can download this file via `curl` or other tools without credentials:
    ///
    /// ```shell
    /// curl "https://s3.amazonaws.com/examplebucket/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=access_key_id/20130721/us-east-1/s3/aws4_request&X-Amz-Date=20130721T201207Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host&X-Amz-Signature=<signature-value>" -O /tmp/test.txt
    /// ```
    pub async fn presign_read(&self, path: &str, expire: Duration) -> Result<PresignedRequest> {
        self.presign_read_with(path, expire).await
    }

    /// Presign an operation for read with extra options.
    ///
    /// # Options
    ///
    /// Visit [`options::ReadOptions`] for all available options.
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal_core::Operator;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op
    ///         .presign_read_with("test.txt", Duration::from_secs(3600))
    ///         .override_content_type("text/plain")
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn presign_read_with(
        &self,
        path: &str,
        expire: Duration,
    ) -> FuturePresignRead<impl Future<Output = Result<PresignedRequest>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            (options::ReadOptions::default(), expire),
            Self::presign_read_inner,
        )
    }

    /// Presign an operation for read with additional options.
    ///
    /// # Options
    ///
    /// Visit [`options::ReadOptions`] for all available options.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use opendal_core::Operator;
    /// use opendal_core::options;
    /// use std::time::Duration;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op.presign_read_options(
    ///         "file",
    ///         Duration::from_secs(3600),
    ///         options::ReadOptions {
    ///             override_content_disposition: Some("attachment; filename=\"othertext.txt\"".to_string()),
    ///             ..Default::default()
    ///         }
    ///     ).await?;
    ///     let req = http::Request::builder()
    ///         .method(signed_req.method())
    ///         .uri(signed_req.uri())
    ///         .body(())?;
    ///
    /// #    Ok(())
    /// # }
    /// ```
    pub async fn presign_read_options(
        &self,
        path: &str,
        expire: Duration,
        opts: options::ReadOptions,
    ) -> Result<PresignedRequest> {
        let path = normalize_path(path);
        Self::presign_read_inner(
            self.context().clone(),
            self.service().clone(),
            path,
            (opts, expire),
        )
        .await
    }

    #[inline]
    async fn presign_read_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        (opts, expire): (options::ReadOptions, Duration),
    ) -> Result<PresignedRequest> {
        let (range, op_read, _) = opts.into();
        let op = OpPresign::new(PresignOperation::Read(range, op_read), expire);
        let rp = srv.presign(&ctx, &path, op).await?;
        Ok(rp.into_presigned_request())
    }

    /// Presign an operation for write.
    ///
    /// # Notes
    ///
    /// ## Extra Options
    ///
    /// `presign_write` is a wrapper of [`Self::presign_write_with`] without any options. To use
    /// extra options like `content_type`, please use [`Self::presign_write_with`] or
    /// [`Self::presign_write_options`] instead.
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use anyhow::Result;
    /// use opendal_core::Operator;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op
    ///         .presign_write("test.txt", Duration::from_secs(3600))
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// - `signed_req.method()`: `PUT`
    /// - `signed_req.uri()`: `https://s3.amazonaws.com/examplebucket/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=access_key_id/20130721/us-east-1/s3/aws4_request&X-Amz-Date=20130721T201207Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host&X-Amz-Signature=<signature-value>`
    /// - `signed_req.headers()`: `{ "host": "s3.amazonaws.com" }`
    ///
    /// We can upload file as this file via `curl` or other tools without credential:
    ///
    /// ```shell
    /// curl -X PUT "https://s3.amazonaws.com/examplebucket/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=access_key_id/20130721/us-east-1/s3/aws4_request&X-Amz-Date=20130721T201207Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host&X-Amz-Signature=<signature-value>" -d "Hello, World!"
    /// ```
    pub async fn presign_write(&self, path: &str, expire: Duration) -> Result<PresignedRequest> {
        self.presign_write_with(path, expire).await
    }

    /// Presign an operation for write with extra options.
    ///
    /// # Options
    ///
    /// Visit [`options::WriteOptions`] for all available options.
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use anyhow::Result;
    /// use opendal_core::Operator;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op
    ///         .presign_write_with("test", Duration::from_secs(3600))
    ///         .cache_control("no-store")
    ///         .await?;
    ///     let req = http::Request::builder()
    ///         .method(signed_req.method())
    ///         .uri(signed_req.uri())
    ///         .body(())?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn presign_write_with(
        &self,
        path: &str,
        expire: Duration,
    ) -> FuturePresignWrite<impl Future<Output = Result<PresignedRequest>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            (options::WriteOptions::default(), expire),
            Self::presign_write_inner,
        )
    }

    /// Presign an operation for write with additional options.
    ///
    /// # Options
    ///
    /// Check [`options::WriteOptions`] for all available options.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use opendal_core::Operator;
    /// use opendal_core::options;
    /// use std::time::Duration;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op.presign_write_options(
    ///         "file",
    ///         Duration::from_secs(3600),
    ///         options::WriteOptions {
    ///             content_type: Some("application/json".to_string()),
    ///             cache_control: Some("max-age=3600".to_string()),
    ///             if_not_exists: true,
    ///             ..Default::default()
    ///         }
    ///     ).await?;
    ///     let req = http::Request::builder()
    ///         .method(signed_req.method())
    ///         .uri(signed_req.uri())
    ///         .body(())?;
    ///
    /// #    Ok(())
    /// # }
    /// ```
    pub async fn presign_write_options(
        &self,
        path: &str,
        expire: Duration,
        opts: options::WriteOptions,
    ) -> Result<PresignedRequest> {
        let path = normalize_path(path);
        Self::presign_write_inner(
            self.context().clone(),
            self.service().clone(),
            path,
            (opts, expire),
        )
        .await
    }

    #[inline]
    async fn presign_write_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        (opts, expire): (options::WriteOptions, Duration),
    ) -> Result<PresignedRequest> {
        let (op_write, _) = OpWrite::from_options(&srv.capability(), opts)
            .map_err(|err| err.with_context("service", srv.info().scheme()))?;
        let op = OpPresign::new(op_write, expire);
        let rp = srv.presign(&ctx, &path, op).await?;
        Ok(rp.into_presigned_request())
    }

    /// Presign an operation for delete.
    ///
    /// # Notes
    ///
    /// ## Extra Options
    ///
    /// `presign_delete` is a wrapper of [`Self::presign_delete_with`] without any options.
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// use anyhow::Result;
    /// use opendal_core::Operator;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op
    ///         .presign_delete("test.txt", Duration::from_secs(3600))
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// - `signed_req.method()`: `DELETE`
    /// - `signed_req.uri()`: `https://s3.amazonaws.com/examplebucket/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=access_key_id/20130721/us-east-1/s3/aws4_request&X-Amz-Date=20130721T201207Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host&X-Amz-Signature=<signature-value>`
    /// - `signed_req.headers()`: `{ "host": "s3.amazonaws.com" }`
    ///
    /// We can delete file as this file via `curl` or other tools without credential:
    ///
    /// ```shell
    /// curl -X DELETE "https://s3.amazonaws.com/examplebucket/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=access_key_id/20130721/us-east-1/s3/aws4_request&X-Amz-Date=20130721T201207Z&X-Amz-Expires=86400&X-Amz-SignedHeaders=host&X-Amz-Signature=<signature-value>"
    /// ```
    pub async fn presign_delete(&self, path: &str, expire: Duration) -> Result<PresignedRequest> {
        self.presign_delete_with(path, expire).await
    }

    /// Presign an operation for delete without extra options.
    pub fn presign_delete_with(
        &self,
        path: &str,
        expire: Duration,
    ) -> FuturePresignDelete<impl Future<Output = Result<PresignedRequest>>> {
        let path = normalize_path(path);

        OperatorFuture::new(
            self.context().clone(),
            self.service().clone(),
            path,
            (options::DeleteOptions::default(), expire),
            Self::presign_delete_inner,
        )
    }

    /// Presign an operation for delete with additional options.
    ///
    /// # Options
    ///
    /// Visit [`options::DeleteOptions`] for all available options.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use opendal_core::Operator;
    /// use opendal_core::options;
    /// use std::time::Duration;
    ///
    /// async fn test(op: Operator) -> Result<()> {
    ///     let signed_req = op.presign_delete_options(
    ///         "path/to/file",
    ///         Duration::from_secs(3600),
    ///         options::DeleteOptions {
    ///             ..Default::default()
    ///         }
    ///     ).await?;
    ///     let req = http::Request::builder()
    ///         .method(signed_req.method())
    ///         .uri(signed_req.uri())
    ///         .body(())?;
    ///
    /// #    Ok(())
    /// # }
    /// ```
    pub async fn presign_delete_options(
        &self,
        path: &str,
        expire: Duration,
        opts: options::DeleteOptions,
    ) -> Result<PresignedRequest> {
        let path = normalize_path(path);
        Self::presign_delete_inner(
            self.context().clone(),
            self.service().clone(),
            path,
            (opts, expire),
        )
        .await
    }

    #[inline]
    async fn presign_delete_inner(
        ctx: OperationContext,
        srv: Servicer,
        path: String,
        (opts, expire): (options::DeleteOptions, Duration),
    ) -> Result<PresignedRequest> {
        let op_delete = OpDelete::from_options(&srv.capability(), opts)
            .map_err(|err| err.with_context("service", srv.info().scheme()))?;
        let op = OpPresign::new(op_delete, expire);
        let rp = srv.presign(&ctx, &path, op).await?;
        Ok(rp.into_presigned_request())
    }
}
