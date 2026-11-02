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

use std::time::Duration;

use tokio::runtime::Handle;

use crate::Operator as AsyncOperator;
use crate::raw::PresignedRequest;
use crate::types::IntoOperatorUri;
use crate::*;

/// Use OpenDAL in blocking context.
///
/// # Notes
///
/// blocking::Operator is a wrapper around [`AsyncOperator`]. It calls async runtimes' `block_on` API to spawn blocking tasks.
/// Please avoid using blocking::Operator in async context.
///
/// # Examples
///
/// ## Init in async context
///
/// blocking::Operator will use current async context's runtime to handle the async calls.
///
/// This is just for initialization. You must use `blocking::Operator` in blocking context.
///
/// ```rust,no_run
/// # use opendal_core::services;
/// # use opendal_core::blocking;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create fs backend builder.
///     let builder = services::Memory::default();
///     let op = Operator::new(builder)?;
///
///     // Build an `blocking::Operator` with blocking layer to start operating the storage.
///     let _: blocking::Operator = blocking::Operator::new(op)?;
///
///     Ok(())
/// }
/// ```
///
/// ## In async context with blocking functions
///
/// If `blocking::Operator` is called in blocking function, please fetch a [`tokio::runtime::EnterGuard`]
/// first. You can use [`Handle::try_current`] first to get the handle and then call [`Handle::enter`].
/// This often happens in the case that async function calls blocking function.
///
/// ```rust,no_run
/// # use opendal_core::services;
/// # use opendal_core::blocking;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let _ = blocking_fn()?;
///     Ok(())
/// }
///
/// fn blocking_fn() -> Result<blocking::Operator> {
///     // Create fs backend builder.
///     let builder = services::Memory::default();
///     let op = Operator::new(builder)?;
///
///     let handle = tokio::runtime::Handle::try_current().unwrap();
///     let _guard = handle.enter();
///     // Build an `blocking::Operator` to start operating the storage.
///     let op: blocking::Operator = blocking::Operator::new(op)?;
///     Ok(op)
/// }
/// ```
///
/// ## In blocking context
///
/// In a pure blocking context, we can create a runtime and use it to create the `blocking::Operator`.
///
/// > The following code uses a global statically created runtime as an example, please manage the
/// > runtime on demand.
///
/// ```rust,no_run
/// # use std::sync::LazyLock;
/// # use opendal_core::services;
/// # use opendal_core::blocking;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
///
/// static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
///     tokio::runtime::Builder::new_multi_thread()
///         .enable_all()
///         .build()
///         .unwrap()
/// });
///
/// fn main() -> Result<()> {
///     // Create fs backend builder.
///     let builder = services::Memory::default();
///     let op = Operator::new(builder)?;
///
///     // Fetch the `EnterGuard` from global runtime.
///     let _guard = RUNTIME.enter();
///     // Build an `blocking::Operator` with blocking layer to start operating the storage.
///     let _: blocking::Operator = blocking::Operator::new(op)?;
///
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Operator {
    handle: tokio::runtime::Handle,
    op: AsyncOperator,
}

impl Operator {
    /// Create a new `BlockingLayer` with the current runtime's handle
    pub fn new(op: AsyncOperator) -> Result<Self> {
        Ok(Self {
            handle: Handle::try_current()
                .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to get current handle"))?,
            op,
        })
    }

    /// Spawn a future onto the runtime's worker pool and block until it
    /// completes.
    ///
    /// Unlike [`Handle::block_on`] which polls the future on the **calling**
    /// thread's stack, this method runs the future on a tokio worker thread
    /// (typically 8 MB stack) and only uses the calling thread to wait for
    /// the result.  This avoids stack overflows when the async state machine
    /// is deeply nested (e.g. HF/XET uploads driven from a JVM thread with
    /// a 1 MB default stack).
    fn spawn_block<F>(&self, f: F) -> Result<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.handle.block_on(self.handle.spawn(f)).map_err(|err| {
            Error::new(ErrorKind::Unexpected, "blocking task failed").set_source(err)
        })
    }

    /// Create a blocking operator from URI based configuration.
    pub fn from_uri(uri: impl IntoOperatorUri) -> Result<Self> {
        let op = AsyncOperator::from_uri(uri)?;
        Self::new(op)
    }

    /// Get information of underlying accessor.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// use opendal_core::blocking;
    /// # use anyhow::Result;
    /// use opendal_core::blocking::Operator;
    ///
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// let info = op.info();
    /// # Ok(())
    /// # }
    /// ```
    pub fn info(&self) -> OperatorInfo {
        self.op.info()
    }
}

/// # Operator blocking API.
impl Operator {
    /// Create a presigned request for stat.
    ///
    /// See [`Operator::presign_stat`] for more details.
    pub fn presign_stat(&self, path: &str, expire: Duration) -> Result<PresignedRequest> {
        self.handle.block_on(self.op.presign_stat(path, expire))
    }

    /// Create a presigned request for stat with additional options.
    pub fn presign_stat_options(
        &self,
        path: &str,
        expire: Duration,
        opts: options::StatOptions,
    ) -> Result<PresignedRequest> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.presign_stat_options(&path, expire, opts).await })?
    }

    /// Create a presigned request for read.
    ///
    /// See [`Operator::presign_read`] for more details.
    pub fn presign_read(&self, path: &str, expire: Duration) -> Result<PresignedRequest> {
        self.handle.block_on(self.op.presign_read(path, expire))
    }

    /// Create a presigned request for read with additional options.
    pub fn presign_read_options(
        &self,
        path: &str,
        expire: Duration,
        opts: options::ReadOptions,
    ) -> Result<PresignedRequest> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.presign_read_options(&path, expire, opts).await })?
    }

    /// Create a presigned request for write.
    ///
    /// See [`Operator::presign_write`] for more details.
    pub fn presign_write(&self, path: &str, expire: Duration) -> Result<PresignedRequest> {
        self.handle.block_on(self.op.presign_write(path, expire))
    }

    /// Create a presigned request for write with additional options.
    pub fn presign_write_options(
        &self,
        path: &str,
        expire: Duration,
        opts: options::WriteOptions,
    ) -> Result<PresignedRequest> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.presign_write_options(&path, expire, opts).await })?
    }

    /// Create a presigned request for delete.
    ///
    /// See [`Operator::presign_delete`] for more details.
    pub fn presign_delete(&self, path: &str, expire: Duration) -> Result<PresignedRequest> {
        self.handle.block_on(self.op.presign_delete(path, expire))
    }

    /// Create a presigned request for delete with additional options.
    pub fn presign_delete_options(
        &self,
        path: &str,
        expire: Duration,
        opts: options::DeleteOptions,
    ) -> Result<PresignedRequest> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.presign_delete_options(&path, expire, opts).await })?
    }

    /// Get given path's metadata.
    ///
    /// # Behavior
    ///
    /// ## Services that support `create_dir`
    ///
    /// `test` and `test/` may vary in some services such as S3. However, on a local file system,
    /// they're identical. Therefore, the behavior of `stat("test")` and `stat("test/")` might differ
    /// in certain edge cases. Always use `stat("test/")` when you need to access a directory if possible.
    ///
    /// Here are the behavior list:
    ///
    /// | Case                   | Path            | Result                                     |
    /// |------------------------|-----------------|--------------------------------------------|
    /// | stat existing dir      | `abc/`          | Metadata with dir mode                     |
    /// | stat existing file     | `abc/def_file`  | Metadata with file mode                    |
    /// | stat dir without `/`   | `abc/def_dir`   | Error `NotFound` or metadata with dir mode |
    /// | stat file with `/`     | `abc/def_file/` | Error `NotFound`                           |
    /// | stat not existing path | `xyz`           | Error `NotFound`                           |
    ///
    /// Refer to [RFC: List Prefix](https://github.com/apache/opendal/blob/main/core/core/src/docs/rfcs/3243_list_prefix.md)
    /// for more details.
    ///
    /// ## Services that not support `create_dir`
    ///
    /// For services that not support `create_dir`, `stat("test/")` will return `NotFound` even
    /// when `test/abc` exists since the service won't have the concept of dir. There is nothing
    /// we can do about this.
    ///
    /// # Examples
    ///
    /// ## Check if file exists
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    /// use opendal_core::ErrorKind;
    /// #
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// if let Err(e) = op.stat("test") {
    ///     if e.kind() == ErrorKind::NotFound {
    ///         println!("file not exist")
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn stat(&self, path: &str) -> Result<Metadata> {
        self.stat_options(path, options::StatOptions::default())
    }

    /// Get given path's metadata with extra options.
    ///
    /// # Behavior
    ///
    /// ## Services that support `create_dir`
    ///
    /// `test` and `test/` may vary in some services such as S3. However, on a local file system,
    /// they're identical. Therefore, the behavior of `stat("test")` and `stat("test/")` might differ
    /// in certain edge cases. Always use `stat("test/")` when you need to access a directory if possible.
    ///
    /// Here are the behavior list:
    ///
    /// | Case                   | Path            | Result                                     |
    /// |------------------------|-----------------|--------------------------------------------|
    /// | stat existing dir      | `abc/`          | Metadata with dir mode                     |
    /// | stat existing file     | `abc/def_file`  | Metadata with file mode                    |
    /// | stat dir without `/`   | `abc/def_dir`   | Error `NotFound` or metadata with dir mode |
    /// | stat file with `/`     | `abc/def_file/` | Error `NotFound`                           |
    /// | stat not existing path | `xyz`           | Error `NotFound`                           |
    ///
    /// Refer to [RFC: List Prefix](https://github.com/apache/opendal/blob/main/core/core/src/docs/rfcs/3243_list_prefix.md)
    /// for more details.
    ///
    /// ## Services that not support `create_dir`
    ///
    /// For services that not support `create_dir`, `stat("test/")` will return `NotFound` even
    /// when `test/abc` exists since the service won't have the concept of dir. There is nothing
    /// we can do about this.
    pub fn stat_options(&self, path: &str, opts: options::StatOptions) -> Result<Metadata> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.stat_options(&path, opts).await })?
    }

    /// Check if this path exists or not.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anyhow::Result;
    /// use opendal_core::blocking;
    /// use opendal_core::blocking::Operator;
    /// fn test(op: blocking::Operator) -> Result<()> {
    ///     let _ = op.exists("test")?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn exists(&self, path: &str) -> Result<bool> {
        let r = self.stat(path);
        match r {
            Ok(_) => Ok(true),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(false),
                _ => Err(err),
            },
        }
    }

    /// Create a dir at given path.
    ///
    /// # Notes
    ///
    /// To indicate that a path is a directory, it is compulsory to include
    /// a trailing / in the path. Failure to do so may result in
    /// `NotADirectory` error being returned by OpenDAL.
    ///
    /// # Behavior
    ///
    /// - Create on existing dir will succeed.
    /// - Create dir is always recursive, works like `mkdir -p`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal_core::Result;
    /// use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    /// # use futures::TryStreamExt;
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// op.create_dir("path/to/dir/")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_dir(&self, path: &str) -> Result<()> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.create_dir(&path).await })?
    }

    /// Read the whole path into a bytes.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`blocking::Operator::reader`]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal_core::Result;
    /// use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    /// #
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// let bs = op.read("path/to/file")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn read(&self, path: &str) -> Result<Buffer> {
        self.read_options(path, options::ReadOptions::default())
    }

    /// Read the whole path into a bytes with extra options.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`blocking::Operator::reader`]
    pub fn read_options(&self, path: &str, opts: options::ReadOptions) -> Result<Buffer> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.read_options(&path, opts).await })?
    }

    /// Create a new reader which can read the whole path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal_core::Result;
    /// use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    /// # use futures::TryStreamExt;
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// let r = op.reader("path/to/file")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn reader(&self, path: &str) -> Result<blocking::Reader> {
        self.reader_options(path, options::ReaderOptions::default())
    }

    /// Create a new reader with extra options
    pub fn reader_options(
        &self,
        path: &str,
        opts: options::ReaderOptions,
    ) -> Result<blocking::Reader> {
        let r = self.handle.block_on(self.op.reader_options(path, opts))?;
        Ok(blocking::Reader::new(self.handle.clone(), r))
    }

    /// Write bytes into given path.
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal_core::Result;
    /// # use opendal_core::blocking::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// use bytes::Bytes;
    /// use opendal_core::blocking;
    ///
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// op.write("path/to/file", vec![0; 4096])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn write(&self, path: &str, bs: impl Into<Buffer>) -> Result<Metadata> {
        self.write_options(path, bs, options::WriteOptions::default())
    }

    /// Write data with options.
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    pub fn write_options(
        &self,
        path: &str,
        bs: impl Into<Buffer>,
        opts: options::WriteOptions,
    ) -> Result<Metadata> {
        let op = self.op.clone();
        let path = path.to_string();
        let bs = bs.into();
        self.spawn_block(async move { op.write_options(&path, bs, opts).await })?
    }

    /// Write multiple bytes into given path.
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal_core::Result;
    /// # use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// use bytes::Bytes;
    ///
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// let mut w = op.writer("path/to/file")?;
    /// w.write(vec![0; 4096])?;
    /// w.write(vec![1; 4096])?;
    /// w.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn writer(&self, path: &str) -> Result<blocking::Writer> {
        self.writer_options(path, options::WriteOptions::default())
    }

    /// Create a new writer with extra options
    pub fn writer_options(
        &self,
        path: &str,
        opts: options::WriteOptions,
    ) -> Result<blocking::Writer> {
        let w = self.handle.block_on(self.op.writer_options(path, opts))?;
        Ok(blocking::Writer::new(self.handle.clone(), w))
    }

    /// Copy a file from `from` to `to`.
    ///
    /// # Notes
    ///
    /// - `from` and `to` must be a file.
    /// - `to` will be overwritten if it exists.
    /// - If `from` and `to` are the same, nothing will happen.
    /// - `copy` is idempotent. For same `from` and `to` input, the result will be the same.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal_core::Result;
    /// use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    ///
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// op.copy("path/to/file", "path/to/file2")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn copy(&self, from: &str, to: &str) -> Result<Metadata> {
        self.copy_options(from, to, options::CopyOptions::default())
    }

    /// Copy a file from `from` to `to` with additional options.
    pub fn copy_options(
        &self,
        from: &str,
        to: &str,
        opts: options::CopyOptions,
    ) -> Result<Metadata> {
        let op = self.op.clone();
        let from = from.to_string();
        let to = to.to_string();
        self.spawn_block(async move { op.copy_options(&from, &to, opts).await })?
    }

    /// Create a copier from `from` to `to`.
    ///
    /// This function creates a new [`blocking::Copier`] that implements
    /// `Iterator<Item = Result<usize>>`.
    pub fn copier(&self, from: &str, to: &str) -> Result<blocking::Copier> {
        self.copier_options(from, to, options::CopyOptions::default())
    }

    /// Create a copier from `from` to `to` with additional options.
    pub fn copier_options(
        &self,
        from: &str,
        to: &str,
        opts: options::CopyOptions,
    ) -> Result<blocking::Copier> {
        let copier = self
            .handle
            .block_on(self.op.copier_options(from, to, opts))?;
        Ok(blocking::Copier::new(self.handle.clone(), copier))
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
    /// use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    ///
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// op.rename("path/to/file", "path/to/file2")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.rename_options(from, to, options::RenameOptions::default())
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
    /// use opendal_core::blocking;
    /// use opendal_core::options::RenameOptions;
    /// use opendal_core::Result;
    ///
    /// fn rename_with_options(op: blocking::Operator) -> Result<()> {
    ///     let mut opts = RenameOptions::default();
    ///     opts.if_not_exists = true;
    ///     op.rename_options("path/to/file", "path/to/file2", opts)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn rename_options(&self, from: &str, to: &str, opts: options::RenameOptions) -> Result<()> {
        let op = self.op.clone();
        let from = from.to_string();
        let to = to.to_string();
        self.spawn_block(async move { op.rename_options(&from, &to, opts).await })?
    }

    /// Restore the given path from its latest deleted state.
    pub fn restore(&self, path: &str) -> Result<()> {
        self.restore_options(path, options::RestoreOptions::default())
    }

    /// Restore the given path with additional options.
    pub fn restore_options(&self, path: &str, opts: options::RestoreOptions) -> Result<()> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.restore_options(&path, opts).await })?
    }

    /// Delete given path.
    ///
    /// # Notes
    ///
    /// - Delete not existing error won't return errors.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # use futures::io;
    /// use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// op.delete("path/to/file")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete(&self, path: &str) -> Result<()> {
        self.delete_options(path, options::DeleteOptions::default())
    }

    /// Delete given path with options.
    ///
    /// # Notes
    ///
    /// - Delete not existing error won't return errors.
    pub fn delete_options(&self, path: &str, opts: options::DeleteOptions) -> Result<()> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.delete_options(&path, opts).await })?
    }

    /// Delete an infallible iterator of paths.
    ///
    /// Also see:
    ///
    /// - [`blocking::Operator::delete_try_iter`]: delete an fallible iterator of paths.
    pub fn delete_iter<I, D>(&self, iter: I) -> Result<()>
    where
        I: IntoIterator<Item = D>,
        D: IntoDeleteInput,
    {
        self.handle.block_on(self.op.delete_iter(iter))
    }

    /// Delete a fallible iterator of paths.
    ///
    /// Also see:
    ///
    /// - [`blocking::Operator::delete_iter`]: delete an infallible iterator of paths.
    pub fn delete_try_iter<I, D>(&self, try_iter: I) -> Result<()>
    where
        I: IntoIterator<Item = Result<D>>,
        D: IntoDeleteInput,
    {
        self.handle.block_on(self.op.delete_try_iter(try_iter))
    }

    /// Create a [`blocking::Deleter`] to continuously remove content from
    /// storage.
    ///
    /// It leverages batch deletion capabilities provided by storage services for efficient removal.
    ///
    /// Use [`blocking::Deleter`] directly for more control over the deletion
    /// process.
    pub fn deleter(&self) -> Result<blocking::Deleter> {
        blocking::Deleter::create(
            self.handle.clone(),
            self.handle.block_on(self.op.deleter())?,
        )
    }

    /// Remove the path and all nested dirs and files recursively.
    ///
    /// # Deprecated
    ///
    /// This method is deprecated since v0.55.0. Use [`blocking::Operator::delete_options`] with
    /// `recursive: true` instead.
    ///
    /// ## Migration Example
    ///
    /// Instead of:
    /// ```ignore
    /// op.remove_all("path/to/dir")?;
    /// ```
    ///
    /// Use:
    /// ```ignore
    /// use opendal_core::options::DeleteOptions;
    /// op.delete_options("path/to/dir", DeleteOptions {
    ///     recursive: true,
    ///     ..Default::default()
    /// })?;
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
    /// use opendal_core::blocking;
    /// # use opendal_core::blocking::Operator;
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// op.remove_all("path/to/dir")?;
    /// # Ok(())
    /// # }
    /// ```
    #[deprecated(
        since = "0.55.0",
        note = "Use `delete_options` with `recursive: true` instead"
    )]
    #[allow(deprecated)]
    pub fn remove_all(&self, path: &str) -> Result<()> {
        self.delete_options(
            path,
            options::DeleteOptions {
                recursive: true,
                ..Default::default()
            },
        )
    }

    /// List entries whose paths start with the given prefix `path`.
    ///
    /// # Semantics
    ///
    /// - Listing is **prefix-based**; it doesn't require the parent directory to exist.
    /// - If `path` itself exists, it is returned as an entry along with prefixed children.
    /// - If `path` is missing but deeper objects exist, the list succeeds and returns those prefixed entries instead of an error.
    /// - Set `recursive` in [`options::ListOptions`] via [`list_options`](Self::list_options) to walk all descendants; the default returns only immediate children when delimiter is supported.
    ///
    /// ## Streaming List
    ///
    /// This function materializes the full result in memory. For large listings, prefer [`blocking::Operator::lister`] to stream entries.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use opendal_core::blocking;
    /// use opendal_core::blocking::Operator;
    /// use opendal_core::EntryMode;
    /// #  fn test(op: blocking::Operator) -> Result<()> {
    /// let mut entries = op.list("path/to/dir/")?;
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
    pub fn list(&self, path: &str) -> Result<Vec<Entry>> {
        self.list_options(path, options::ListOptions::default())
    }

    /// List entries whose paths start with the given prefix `path` with additional options.
    ///
    /// # Semantics
    ///
    /// Inherits the prefix semantics described in [`Operator::list`] (blocking variant). It returns `path` itself if present and tolerates missing parents when prefixed objects exist.
    ///
    /// ## Streaming List
    ///
    /// This function materializes the full result in memory. For large listings, prefer [`blocking::Operator::lister`] to stream entries.
    ///
    /// ## Options
    ///
    /// See [`options::ListOptions`] for the full set. Common knobs: traversal (`recursive`), pagination (`limit`, `start_after`), and versioning (`versions`, `deleted`).
    pub fn list_options(&self, path: &str, opts: options::ListOptions) -> Result<Vec<Entry>> {
        let op = self.op.clone();
        let path = path.to_string();
        self.spawn_block(async move { op.list_options(&path, opts).await })?
    }

    /// Create a streaming lister for entries whose paths start with the given prefix `path`.
    ///
    /// This function creates a new [`blocking::Lister`]; dropping it stops
    /// listing.
    ///
    /// # Semantics
    ///
    /// Shares the same prefix semantics as [`blocking::Operator::list`]: parent directory is optional; `path` itself is yielded if present; missing parents with deeper objects are accepted.
    ///
    /// ## Options
    ///
    /// Accepts the same [`options::ListOptions`] as [`list_options`](Self::list_options): traversal (`recursive`), pagination (`limit`, `start_after`), and versioning (`versions`, `deleted`).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # use futures::io;
    /// use futures::TryStreamExt;
    /// use opendal_core::blocking;
    /// use opendal_core::blocking::Operator;
    /// use opendal_core::EntryMode;
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// let mut ds = op.lister("path/to/dir/")?;
    /// for de in ds {
    ///     let de = de?;
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
    pub fn lister(&self, path: &str) -> Result<blocking::Lister> {
        self.lister_options(path, options::ListOptions::default())
    }

    /// List entries under a prefix as an iterator with options.
    ///
    /// This function creates a new handle to stream entries and inherits the prefix semantics of [`blocking::Operator::list`].
    ///
    /// ## Options
    ///
    /// Same as [`lister`](Self::lister); see [`options::ListOptions`] for traversal, pagination, and versioning knobs.
    pub fn lister_options(
        &self,
        path: &str,
        opts: options::ListOptions,
    ) -> Result<blocking::Lister> {
        let l = self.handle.block_on(self.op.lister_options(path, opts))?;
        Ok(blocking::Lister::new(self.handle.clone(), l))
    }

    /// Check if this operator can work correctly.
    ///
    /// We will send a `list` request to path and return any errors we met.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// use opendal_core::blocking;
    /// use opendal_core::blocking::Operator;
    /// use opendal_core::ErrorKind;
    ///
    /// # fn test(op: blocking::Operator) -> Result<()> {
    /// op.check()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn check(&self) -> Result<()> {
        let mut ds = self.lister("/")?;

        match ds.next() {
            Some(Err(e)) if e.kind() != ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

impl From<Operator> for AsyncOperator {
    fn from(val: Operator) -> Self {
        val.op
    }
}
