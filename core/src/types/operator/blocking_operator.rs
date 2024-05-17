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

use bytes::Buf;
use bytes::Bytes;

use super::operator_functions::*;
use crate::raw::*;
use crate::*;

/// BlockingOperator is the entry for all public blocking APIs.
///
/// Read [`concepts`][docs::concepts] for know more about [`Operator`].
///
/// # Examples
///
/// ## Init backends
///
/// Read more backend init examples in [`services`]
///
/// ```rust,no_run
/// # use anyhow::Result;
/// use opendal::services::Fs;
/// use opendal::BlockingOperator;
/// use opendal::Operator;
///
/// fn main() -> Result<()> {
///     // Create fs backend builder.
///     let mut builder = Fs::default();
///     // Set the root for fs, all operations will happen under this root.
///     //
///     // NOTE: the root must be absolute path.
///     builder.root("/tmp");
///
///     // Build an `BlockingOperator` to start operating the storage.
///     let _: BlockingOperator = Operator::new(builder)?.finish().blocking();
///
///     Ok(())
/// }
/// ```
///
/// ## Init backends with blocking layer
///
/// Some services like s3, gcs doesn't have native blocking supports, we can use [`layers::BlockingLayer`]
/// to wrap the async operator to make it blocking.
#[cfg_attr(feature = "layers-blocking", doc = "```rust")]
#[cfg_attr(not(feature = "layers-blocking"), doc = "```ignore")]
/// # use anyhow::Result;
/// use opendal::layers::BlockingLayer;
/// use opendal::services::S3;
/// use opendal::BlockingOperator;
/// use opendal::Operator;
///
/// async fn test() -> Result<()> {
///     // Create fs backend builder.
///     let mut builder = S3::default();
///     builder.bucket("test");
///     builder.region("us-east-1");
///
///     // Build an `BlockingOperator` with blocking layer to start operating the storage.
///     let _: BlockingOperator = Operator::new(builder)?
///         .layer(BlockingLayer::create()?)
///         .finish()
///         .blocking();
///
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct BlockingOperator {
    accessor: Accessor,

    limit: usize,
}

impl BlockingOperator {
    pub(super) fn inner(&self) -> &Accessor {
        &self.accessor
    }

    /// create a new blocking operator from inner accessor.
    ///
    /// # Note
    /// default batch limit is 1000.
    pub(crate) fn from_inner(accessor: Accessor) -> Self {
        let limit = accessor
            .info()
            .full_capability()
            .batch_max_operations
            .unwrap_or(1000);
        Self { accessor, limit }
    }

    /// Get current operator's limit
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Specify the batch limit.
    ///
    /// Default: 1000
    pub fn with_limit(&self, limit: usize) -> Self {
        let mut op = self.clone();
        op.limit = limit;
        op
    }

    /// Get information of underlying accessor.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use anyhow::Result;
    /// use opendal::BlockingOperator;
    ///
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let info = op.info();
    /// # Ok(())
    /// # }
    /// ```
    pub fn info(&self) -> OperatorInfo {
        OperatorInfo::new(self.accessor.info())
    }
}

/// # Operator blocking API.
impl BlockingOperator {
    /// Get given path's metadata.
    ///
    /// # Notes
    ///
    /// For fetch metadata of entries returned by [`Lister`], it's better to use [`list_with`] and
    /// [`lister_with`] with `metakey` query like `Metakey::ContentLength | Metakey::LastModified`
    /// so that we can avoid extra requests.
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
    /// Refer to [RFC: List Prefix][crate::docs::rfcs::rfc_3243_list_prefix] for more details.
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
    /// # use opendal::BlockingOperator;
    /// use opendal::ErrorKind;
    /// #
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// if let Err(e) = op.stat("test") {
    ///     if e.kind() == ErrorKind::NotFound {
    ///         println!("file not exist")
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn stat(&self, path: &str) -> Result<Metadata> {
        self.stat_with(path).call()
    }

    /// Get given path's metadata with extra options.
    ///
    /// # Notes
    ///
    /// For fetch metadata of entries returned by [`Lister`], it's better to use [`list_with`] and
    /// [`lister_with`] with `metakey` query like `Metakey::ContentLength | Metakey::LastModified`
    /// so that we can avoid extra requests.
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
    /// Refer to [RFC: List Prefix][crate::docs::rfcs::rfc_3243_list_prefix] for more details.
    ///
    /// ## Services that not support `create_dir`
    ///
    /// For services that not support `create_dir`, `stat("test/")` will return `NotFound` even
    /// when `test/abc` exists since the service won't have the concept of dir. There is nothing
    /// we can do about this.
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
    /// # use opendal::BlockingOperator;
    /// use opendal::ErrorKind;
    /// #
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// if let Err(e) = op.stat_with("test").if_match("<etag>").call() {
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
    pub fn stat_with(&self, path: &str) -> FunctionStat {
        let path = normalize_path(path);

        FunctionStat(OperatorFunction::new(
            self.inner().clone(),
            path,
            OpStat::default(),
            |inner, path, args| {
                let rp = inner.blocking_stat(&path, args)?;
                let meta = rp.into_metadata();

                Ok(meta)
            },
        ))
    }

    /// Check if this path exists or not.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anyhow::Result;
    /// use opendal::BlockingOperator;
    /// fn test(op: BlockingOperator) -> Result<()> {
    ///     let _ = op.is_exist("test")?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn is_exist(&self, path: &str) -> Result<bool> {
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
    /// # use opendal::Result;
    /// # use opendal::BlockingOperator;
    /// # use futures::TryStreamExt;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// op.create_dir("path/to/dir/")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_dir(&self, path: &str) -> Result<()> {
        let path = normalize_path(path);

        if !validate_path(&path, EntryMode::DIR) {
            return Err(Error::new(
                ErrorKind::NotADirectory,
                "the path trying to create should end with `/`",
            )
            .with_operation("create_dir")
            .with_context("service", self.inner().info().scheme())
            .with_context("path", &path));
        }

        self.inner()
            .blocking_create_dir(&path, OpCreateDir::new())?;

        Ok(())
    }

    /// Read the whole path into a bytes.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`BlockingOperator::reader`]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::Result;
    /// # use opendal::BlockingOperator;
    /// #
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let bs = op.read("path/to/file")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn read(&self, path: &str) -> Result<Buffer> {
        self.read_with(path).call()
    }

    /// Read the whole path into a bytes with extra options.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`BlockingOperator::reader`]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let bs = op.read_with("path/to/file").range(0..10).call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_with(&self, path: &str) -> FunctionRead {
        let path = normalize_path(path);

        FunctionRead(OperatorFunction::new(
            self.inner().clone(),
            path,
            (
                OpRead::default(),
                BytesRange::default(),
                OpReader::default(),
            ),
            |inner, path, (args, range, options)| {
                if !validate_path(&path, EntryMode::FILE) {
                    return Err(
                        Error::new(ErrorKind::IsADirectory, "read path is a directory")
                            .with_operation("BlockingOperator::read_with")
                            .with_context("service", inner.info().scheme().into_static())
                            .with_context("path", &path),
                    );
                }

                let r = BlockingReader::create(inner, &path, args, options)?;
                let buf = r.read(range.to_range())?;
                Ok(buf)
            },
        ))
    }

    /// Create a new reader which can read the whole path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::Result;
    /// # use opendal::BlockingOperator;
    /// # use futures::TryStreamExt;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let r = op.reader("path/to/file")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn reader(&self, path: &str) -> Result<BlockingReader> {
        self.reader_with(path).call()
    }

    /// Create a new reader with extra options
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let r = op
    ///     .reader_with("path/to/file")
    ///     .version("version_id")
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn reader_with(&self, path: &str) -> FunctionReader {
        let path = normalize_path(path);

        FunctionReader(OperatorFunction::new(
            self.inner().clone(),
            path,
            (
                OpRead::default(),
                BytesRange::default(),
                OpReader::default(),
            ),
            |inner, path, (args, _, options)| {
                if !validate_path(&path, EntryMode::FILE) {
                    return Err(
                        Error::new(ErrorKind::IsADirectory, "reader path is a directory")
                            .with_operation("BlockingOperator::reader_with")
                            .with_context("service", inner.info().scheme().into_static())
                            .with_context("path", &path),
                    );
                }

                BlockingReader::create(inner.clone(), &path, args, options)
            },
        ))
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
    /// # use opendal::Result;
    /// # use opendal::BlockingOperator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// use bytes::Bytes;
    ///
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// op.write("path/to/file", vec![0; 4096])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn write(&self, path: &str, bs: impl Into<Bytes>) -> Result<()> {
        self.write_with(path, bs).call()
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
    /// # use opendal::Result;
    /// # use opendal::BlockingOperator;
    ///
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// op.copy("path/to/file", "path/to/file2")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn copy(&self, from: &str, to: &str) -> Result<()> {
        let from = normalize_path(from);

        if !validate_path(&from, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "from path is a directory")
                    .with_operation("BlockingOperator::copy")
                    .with_context("service", self.info().scheme())
                    .with_context("from", from),
            );
        }

        let to = normalize_path(to);

        if !validate_path(&to, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "to path is a directory")
                    .with_operation("BlockingOperator::copy")
                    .with_context("service", self.info().scheme())
                    .with_context("to", to),
            );
        }

        if from == to {
            return Err(
                Error::new(ErrorKind::IsSameFile, "from and to paths are same")
                    .with_operation("BlockingOperator::copy")
                    .with_context("service", self.info().scheme())
                    .with_context("from", from)
                    .with_context("to", to),
            );
        }

        self.inner().blocking_copy(&from, &to, OpCopy::new())?;

        Ok(())
    }

    /// Rename a file from `from` to `to`.
    ///
    /// # Notes
    ///
    /// - `from` and `to` must be a file.
    /// - `to` will be overwritten if it exists.
    /// - If `from` and `to` are the same, a `IsSameFile` error will occur.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::Result;
    /// # use opendal::BlockingOperator;
    ///
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// op.rename("path/to/file", "path/to/file2")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let from = normalize_path(from);

        if !validate_path(&from, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "from path is a directory")
                    .with_operation("BlockingOperator::move")
                    .with_context("service", self.info().scheme())
                    .with_context("from", from),
            );
        }

        let to = normalize_path(to);

        if !validate_path(&to, EntryMode::FILE) {
            return Err(
                Error::new(ErrorKind::IsADirectory, "to path is a directory")
                    .with_operation("BlockingOperator::move")
                    .with_context("service", self.info().scheme())
                    .with_context("to", to),
            );
        }

        if from == to {
            return Err(
                Error::new(ErrorKind::IsSameFile, "from and to paths are same")
                    .with_operation("BlockingOperator::move")
                    .with_context("service", self.info().scheme())
                    .with_context("from", from)
                    .with_context("to", to),
            );
        }

        self.inner().blocking_rename(&from, &to, OpRename::new())?;

        Ok(())
    }

    /// Write data with option described in OpenDAL [RFC-0661][`crate::docs::rfcs::rfc_0661_path_in_accessor`]
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::Result;
    /// # use opendal::BlockingOperator;
    /// use bytes::Bytes;
    ///
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let bs = b"hello, world!".to_vec();
    /// let _ = op
    ///     .write_with("hello.txt", bs)
    ///     .content_type("text/plain")
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_with(&self, path: &str, bs: impl Into<Bytes>) -> FunctionWrite {
        let path = normalize_path(path);

        let bs = bs.into();

        FunctionWrite(OperatorFunction::new(
            self.inner().clone(),
            path,
            (OpWrite::default(), bs),
            |inner, path, (args, mut bs)| {
                if !validate_path(&path, EntryMode::FILE) {
                    return Err(
                        Error::new(ErrorKind::IsADirectory, "write path is a directory")
                            .with_operation("BlockingOperator::write_with")
                            .with_context("service", inner.info().scheme().into_static())
                            .with_context("path", &path),
                    );
                }

                let (_, mut w) = inner.blocking_write(&path, args)?;
                while !bs.is_empty() {
                    let n = w.write(bs.clone().into())?;
                    bs.advance(n);
                }
                w.close()?;

                Ok(())
            },
        ))
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
    /// # use opendal::Result;
    /// # use opendal::BlockingOperator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// use bytes::Bytes;
    ///
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let mut w = op.writer("path/to/file")?;
    /// w.write(vec![0; 4096])?;
    /// w.write(vec![1; 4096])?;
    /// w.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn writer(&self, path: &str) -> Result<BlockingWriter> {
        self.writer_with(path).call()
    }

    /// Create a new reader with extra options
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let mut w = op.writer_with("path/to/file").call()?;
    /// w.write(vec![0; 4096])?;
    /// w.write(vec![1; 4096])?;
    /// w.close()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn writer_with(&self, path: &str) -> FunctionWriter {
        let path = normalize_path(path);

        FunctionWriter(OperatorFunction::new(
            self.inner().clone(),
            path,
            OpWrite::default(),
            |inner, path, args| {
                let path = normalize_path(&path);

                if !validate_path(&path, EntryMode::FILE) {
                    return Err(
                        Error::new(ErrorKind::IsADirectory, "write path is a directory")
                            .with_operation("BlockingOperator::writer_with")
                            .with_context("service", inner.info().scheme().into_static())
                            .with_context("path", &path),
                    );
                }

                BlockingWriter::create(inner.clone(), &path, args)
            },
        ))
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
    /// # use opendal::BlockingOperator;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// op.delete("path/to/file")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete(&self, path: &str) -> Result<()> {
        self.delete_with(path).call()?;

        Ok(())
    }

    /// Delete given path with options.
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
    /// # use opendal::BlockingOperator;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let _ = op
    ///     .delete_with("path/to/file")
    ///     .version("example_version")
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_with(&self, path: &str) -> FunctionDelete {
        let path = normalize_path(path);

        FunctionDelete(OperatorFunction::new(
            self.inner().clone(),
            path,
            OpDelete::new(),
            |inner, path, args| {
                let _ = inner.blocking_delete(&path, args)?;

                Ok(())
            },
        ))
    }

    /// remove will remove files via the given paths.
    ///
    /// remove_via will remove files via the given vector iterators.
    ///
    /// # Notes
    ///
    /// We don't support batch delete now.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::BlockingOperator;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let stream = vec!["abc".to_string(), "def".to_string()].into_iter();
    /// op.remove_via(stream)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn remove_via(&self, input: impl Iterator<Item = String>) -> Result<()> {
        for path in input {
            self.delete(&path)?;
        }
        Ok(())
    }

    /// # Notes
    ///
    /// We don't support batch delete now.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::BlockingOperator;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// op.remove(vec!["abc".to_string(), "def".to_string()])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn remove(&self, paths: Vec<String>) -> Result<()> {
        self.remove_via(paths.into_iter())?;

        Ok(())
    }

    /// Remove the path and all nested dirs and files recursively.
    ///
    /// # Notes
    ///
    /// We don't support batch delete now.
    ///
    /// # Examples
    ///
    /// ```
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::BlockingOperator;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// op.remove_all("path/to/dir")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn remove_all(&self, path: &str) -> Result<()> {
        let meta = match self.stat(path) {
            Ok(metadata) => metadata,

            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),

            Err(e) => return Err(e),
        };

        if meta.mode() != EntryMode::DIR {
            return self.delete(path);
        }

        let obs = self.lister_with(path).recursive(true).call()?;

        for v in obs {
            match v {
                Ok(entry) => {
                    self.inner()
                        .blocking_delete(entry.path(), OpDelete::new())?;
                }
                Err(e) => return Err(e),
            }
        }

        // Remove the directory itself.
        self.delete(path)?;

        Ok(())
    }

    /// List entries that starts with given `path` in parent dir.
    ///
    /// # Notes
    ///
    /// ## Recursively List
    ///
    /// This function only read the children of the given directory. To read
    /// all entries recursively, use `BlockingOperator::list_with("path").recursive(true)`
    /// instead.
    ///
    /// ## Streaming List
    ///
    /// This function will read all entries in the given directory. It could
    /// take very long time and consume a lot of memory if the directory
    /// contains a lot of entries.
    ///
    /// In order to avoid this, you can use [`BlockingOperator::lister`] to list entries in
    /// a streaming way.
    ///
    /// ## Reuse Metadata
    ///
    /// The only metadata that is guaranteed to be available is the `Mode`.
    /// For fetching more metadata, please use [`BlockingOperator::list_with`] and `metakey`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// #  fn test(op: BlockingOperator) -> Result<()> {
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
        self.list_with(path).call()
    }

    /// List entries that starts with given `path` in parent dir. with options.
    ///
    /// # Notes
    ///
    /// ## Streaming List
    ///
    /// This function will read all entries in the given directory. It could
    /// take very long time and consume a lot of memory if the directory
    /// contains a lot of entries.
    ///
    /// In order to avoid this, you can use [`BlockingOperator::lister`] to list entries in
    /// a streaming way.
    ///
    /// ## Reuse Metadata
    ///
    /// The only metadata that is guaranteed to be available is the `Mode`.
    /// For fetching more metadata, please specify the `metakey`.
    ///
    /// # Examples
    ///
    /// ## List entries with prefix
    ///
    /// This function can also be used to list entries in recursive way.
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let mut entries = op.list_with("prefix/").recursive(true).call()?;
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
    ///
    /// ## List entries with metakey for more metadata
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let mut entries = op
    ///     .list_with("dir/")
    ///     .metakey(Metakey::ContentLength | Metakey::LastModified)
    ///     .call()?;
    /// for entry in entries {
    ///     let meta = entry.metadata();
    ///     match meta.mode() {
    ///         EntryMode::FILE => {
    ///             println!(
    ///                 "Handling file {} with size {}",
    ///                 entry.path(),
    ///                 meta.content_length()
    ///             )
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
    pub fn list_with(&self, path: &str) -> FunctionList {
        let path = normalize_path(path);

        FunctionList(OperatorFunction::new(
            self.inner().clone(),
            path,
            OpList::default(),
            |inner, path, args| {
                let lister = BlockingLister::create(inner, &path, args)?;

                lister.collect()
            },
        ))
    }

    /// List entries that starts with given `path` in parent dir.
    ///
    /// This function will create a new [`BlockingLister`] to list entries. Users can stop listing
    /// via dropping this [`Lister`].
    ///
    /// # Notes
    ///
    /// ## Recursively List
    ///
    /// This function only read the children of the given directory. To read
    /// all entries recursively, use [`BlockingOperator::lister_with`] and `delimiter("")`
    /// instead.
    ///
    /// ## Metadata
    ///
    /// The only metadata that is guaranteed to be available is the `Mode`.
    /// For fetching more metadata, please use [`BlockingOperator::lister_with`] and `metakey`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # use futures::io;
    /// use futures::TryStreamExt;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
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
    pub fn lister(&self, path: &str) -> Result<BlockingLister> {
        self.lister_with(path).call()
    }

    /// List entries within a given directory as an iterator with options.
    ///
    /// This function will create a new handle to list entries.
    ///
    /// An error will be returned if given path doesn't end with `/`.
    ///
    /// # Examples
    ///
    /// ## List current dir
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # use futures::io;
    /// use futures::TryStreamExt;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let mut ds = op
    ///     .lister_with("path/to/dir/")
    ///     .limit(10)
    ///     .start_after("start")
    ///     .call()?;
    /// for entry in ds {
    ///     let entry = entry?;
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
    ///
    /// ## List all files recursively
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # use futures::io;
    /// use futures::TryStreamExt;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let mut ds = op.lister_with("path/to/dir/").recursive(true).call()?;
    /// for entry in ds {
    ///     let entry = entry?;
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
    ///
    /// ## List files with required metadata
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # use futures::io;
    /// use futures::TryStreamExt;
    /// use opendal::BlockingOperator;
    /// use opendal::EntryMode;
    /// use opendal::Metakey;
    /// # fn test(op: BlockingOperator) -> Result<()> {
    /// let mut ds = op
    ///     .lister_with("path/to/dir/")
    ///     .metakey(Metakey::ContentLength | Metakey::LastModified)
    ///     .call()?;
    /// for entry in ds {
    ///     let entry = entry?;
    ///     let meta = entry.metadata();
    ///     match meta.mode() {
    ///         EntryMode::FILE => {
    ///             println!(
    ///                 "Handling file {} with size {}",
    ///                 entry.path(),
    ///                 meta.content_length()
    ///             )
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
    pub fn lister_with(&self, path: &str) -> FunctionLister {
        let path = normalize_path(path);

        FunctionLister(OperatorFunction::new(
            self.inner().clone(),
            path,
            OpList::default(),
            |inner, path, args| BlockingLister::create(inner, &path, args),
        ))
    }
}

impl From<BlockingOperator> for Operator {
    fn from(v: BlockingOperator) -> Self {
        Operator::from_inner(v.accessor).with_limit(v.limit)
    }
}
