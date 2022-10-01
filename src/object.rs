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
use std::fmt::Display;
use std::fmt::Formatter;
use std::io::ErrorKind;
use std::io::Result;
use std::ops::RangeBounds;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;

use anyhow::anyhow;
use futures::io;
use futures::io::Cursor;
#[cfg(feature = "serde")]
use serde::Deserialize;
#[cfg(feature = "serde")]
use serde::Serialize;
use time::Duration;
use time::OffsetDateTime;

use crate::error::new_other_object_error;
use crate::io::BytesRead;
use crate::io_util::seekable_read;
#[cfg(feature = "compress")]
use crate::io_util::CompressAlgorithm;
#[cfg(feature = "compress")]
use crate::io_util::DecompressReader;
use crate::io_util::SeekableReader;
use crate::multipart::ObjectMultipart;
use crate::ops::BytesRange;
use crate::ops::OpCreate;
use crate::ops::OpCreateMultipart;
use crate::ops::OpDelete;
use crate::ops::OpList;
use crate::ops::OpPresign;
use crate::ops::OpRead;
use crate::ops::OpStat;
use crate::ops::OpWrite;
use crate::ops::Operation;
use crate::ops::PresignedRequest;
use crate::path::get_basename;
use crate::path::normalize_path;
use crate::path::validate_path;
use crate::Accessor;
use crate::BlockingBytesRead;

/// Handler for all object related operations.
#[derive(Clone, Debug)]
pub struct Object {
    acc: Arc<dyn Accessor>,
    path: String,
}

impl Object {
    /// Creates a new Object with normalized path.
    ///
    /// - All path will be converted into relative path (without any leading `/`)
    /// - Path endswith `/` means it's a dir path.
    /// - Otherwise, it's a file path.
    pub fn new(acc: Arc<dyn Accessor>, path: &str) -> Self {
        Self {
            acc,
            path: normalize_path(path),
        }
    }

    pub(crate) fn accessor(&self) -> Arc<dyn Accessor> {
        self.acc.clone()
    }

    /// ID of object.
    ///
    /// ID is the unique id of object in the underlying backend. In different backend,
    /// the id could have different meaning.
    ///
    /// For example:
    ///
    /// - In `fs`: id is the absolute path of file, like `/path/to/dir/test_object`.
    /// - In `s3`: id is the full object key, like `path/to/dir/test_object`
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal::services::memory;
    /// use opendal::Operator;
    /// use opendal::Scheme;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let op = Operator::from_env(Scheme::Memory)?;
    ///     let id = op.object("test").id();
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn id(&self) -> String {
        format!("{}{}", self.acc.metadata().root(), self.path)
    }

    /// Path of object. Path is relative to operator's root.
    /// Only valid in current operator.
    ///
    /// The value is the same with `Metadata::path()`.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal::services::memory;
    /// use opendal::Operator;
    /// use opendal::Scheme;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let op = Operator::from_env(Scheme::Memory)?;
    ///     let path = op.object("test").path();
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Name of object. Name is the last segment of path.
    ///
    /// If this object is a dir, `Name` MUST endswith `/`
    /// Otherwise, `Name` MUST NOT endswith `/`.
    ///
    /// The value is the same with `Metadata::name()`.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal::services::memory;
    /// use opendal::Operator;
    /// use opendal::Scheme;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let op = Operator::from_env(Scheme::Memory)?;
    ///     let name = op.object("test").name();
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn name(&self) -> &str {
        get_basename(&self.path)
    }

    /// Create an empty object, like using the following linux commands:
    ///
    /// - `touch path/to/file`
    /// - `mkdir path/to/dir/`
    ///
    /// # Behavior
    ///
    /// - Create on existing dir will succeed.
    /// - Create on existing file will overwrite and truncate it.
    ///
    /// # Examples
    ///
    /// ## Create an empty file
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// let _ = o.create().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Create a dir
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/dir/");
    /// let _ = o.create().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self) -> Result<()> {
        if self.path.ends_with('/') {
            self.acc
                .create(self.path(), OpCreate::new(ObjectMode::DIR))
                .await
        } else {
            self.acc
                .create(self.path(), OpCreate::new(ObjectMode::FILE))
                .await
        }
    }

    /// Create an empty object, like using the following linux commands:
    ///
    /// - `touch path/to/file`
    /// - `mkdir path/to/dir/`
    ///
    /// # Behavior
    ///
    /// - Create on existing dir will succeed.
    /// - Create on existing file will overwrite and truncate it.
    ///
    /// # Examples
    ///
    /// ## Create an empty file
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// let _ = o.blocking_create()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Create a dir
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/dir/");
    /// let _ = o.blocking_create()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_create(&self) -> Result<()> {
        if self.path.ends_with('/') {
            self.acc
                .blocking_create(self.path(), OpCreate::new(ObjectMode::DIR))
        } else {
            self.acc
                .blocking_create(self.path(), OpCreate::new(ObjectMode::FILE))
        }
    }

    /// Read the whole object into a bytes.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`Object::reader`]
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// # o.write(vec![0; 4096]).await?;
    /// let bs = o.read().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read(&self) -> Result<Vec<u8>> {
        self.range_read(..).await
    }

    /// Read the whole object into a bytes.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`Object::blocking_reader`]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use opendal::Scheme;
    /// #
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// # o.blocking_write(vec![0; 4096])?;
    /// let bs = o.blocking_read()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_read(&self) -> Result<Vec<u8>> {
        self.blocking_range_read(..)
    }

    /// Read the specified range of object into a bytes.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`Object::range_reader`]
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// # o.write(vec![0; 4096]).await?;
    /// let bs = o.range_read(1024..2048).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn range_read(&self, range: impl RangeBounds<u64>) -> Result<Vec<u8>> {
        if !validate_path(self.path(), ObjectMode::FILE) {
            return Err(new_other_object_error(
                Operation::Read,
                self.path(),
                anyhow!("Is a directory"),
            ));
        }

        let s = self
            .acc
            .read(
                self.path(),
                OpRead::new((range.start_bound(), range.end_bound())),
            )
            .await?;

        let br = BytesRange::from(range);
        let buffer = if let Some(range_size) = br.size() {
            Vec::with_capacity(range_size as usize)
        } else {
            Vec::with_capacity(4 * 1024 * 1024)
        };
        let mut bs = Cursor::new(buffer);

        io::copy(s, &mut bs).await?;

        Ok(bs.into_inner())
    }

    /// Read the specified range of object into a bytes.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`Object::blocking_range_reader`]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// # o.blocking_write(vec![0; 4096])?;
    /// let bs = o.blocking_range_read(1024..2048)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_range_read(&self, range: impl RangeBounds<u64>) -> Result<Vec<u8>> {
        if !validate_path(self.path(), ObjectMode::FILE) {
            return Err(new_other_object_error(
                Operation::Read,
                self.path(),
                anyhow!("Is a directory"),
            ));
        }

        let mut s = self.acc.blocking_read(
            self.path(),
            OpRead::new((range.start_bound(), range.end_bound())),
        )?;

        let br = BytesRange::from(range);
        let mut buffer = if let Some(range_size) = br.size() {
            Vec::with_capacity(range_size as usize)
        } else {
            Vec::with_capacity(4 * 1024 * 1024)
        };

        std::io::copy(&mut s, &mut buffer)?;

        Ok(buffer)
    }

    /// Create a new reader which can read the whole object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// # o.write(vec![0; 4096]).await?;
    /// let r = o.reader().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn reader(&self) -> Result<impl BytesRead> {
        self.range_reader(..).await
    }

    /// Create a new reader which can read the whole object.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// # o.blocking_write(vec![0; 4096])?;
    /// let r = o.blocking_reader()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_reader(&self) -> Result<impl BlockingBytesRead> {
        self.blocking_range_reader(..)
    }

    /// Create a new reader which can read the specified range.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// # o.write(vec![0; 4096]).await?;
    /// let r = o.range_reader(1024..2048).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn range_reader(&self, range: impl RangeBounds<u64>) -> Result<impl BytesRead> {
        if !validate_path(self.path(), ObjectMode::FILE) {
            return Err(new_other_object_error(
                Operation::Read,
                self.path(),
                anyhow!("Is a directory"),
            ));
        }

        self.acc.read(self.path(), OpRead::new(range)).await
    }

    /// Create a new reader which can read the specified range.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// # o.blocking_write(vec![0; 4096])?;
    /// let r = o.blocking_range_reader(1024..2048)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_range_reader(
        &self,
        range: impl RangeBounds<u64>,
    ) -> Result<impl BlockingBytesRead> {
        if !validate_path(self.path(), ObjectMode::FILE) {
            return Err(new_other_object_error(
                Operation::Read,
                self.path(),
                anyhow!("Is a directory"),
            ));
        }

        self.acc.blocking_read(self.path(), OpRead::new(range))
    }

    /// Create a reader which implements AsyncRead and AsyncSeek inside specified range.
    ///
    /// # Notes
    ///
    /// It's not a zero-cost operations. In order to support seeking, we have extra internal
    /// state which maintains the reader contents:
    ///
    /// - Seeking is pure in memory operation.
    /// - Every first read after seeking will start a new read operation on backend.
    ///
    /// This operation is neither async nor returning result, because real IO happens while
    /// users call `read` or `seek`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// # let o = op.object("path/to/file");
    /// let r = o.seekable_reader(1024..2048);
    /// # Ok(())
    /// # }
    /// ```
    pub fn seekable_reader(&self, range: impl RangeBounds<u64>) -> SeekableReader {
        seekable_read(self, range)
    }

    /// Read the whole object into a bytes with auto detected compress algorithm.
    ///
    /// If we can't find the correct algorithm, we return `Ok(None)` instead.
    ///
    /// # Feature
    ///
    /// This function needs to enable feature `compress`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file.gz");
    /// # o.write(vec![0; 4096]).await?;
    /// let bs = o.decompress_read().await?.expect("must read succeed");
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "compress")]
    pub async fn decompress_read(&self) -> Result<Option<Vec<u8>>> {
        let algo = match CompressAlgorithm::from_path(self.path()) {
            None => return Ok(None),
            Some(algo) => algo,
        };

        self.decompress_read_with(algo).await.map(Some)
    }

    /// Create a reader with auto-detected compress algorithm.
    ///
    /// If we can't find the correct algorithm, we will return `Ok(None)`.
    ///
    /// # Feature
    ///
    /// This function needs to enable feature `compress`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file.gz");
    /// # o.write(vec![0; 4096]).await?;
    /// let r = o.decompress_reader().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "compress")]
    pub async fn decompress_reader(&self) -> Result<Option<impl BytesRead>> {
        let algo = match CompressAlgorithm::from_path(self.path()) {
            Some(v) => v,
            None => return Ok(None),
        };

        let r = self.reader().await?;

        Ok(Some(DecompressReader::new(r, algo)))
    }

    /// Read the whole object into a bytes with specific compress algorithm.
    ///
    /// # Feature
    ///
    /// This function needs to enable feature `compress`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::io_util::CompressAlgorithm;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file.gz");
    /// # o.write(vec![0; 4096]).await?;
    /// let bs = o.decompress_read_with(CompressAlgorithm::Gzip).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "compress")]
    pub async fn decompress_read_with(&self, algo: CompressAlgorithm) -> Result<Vec<u8>> {
        let r = self.decompress_reader_with(algo).await?;
        let mut bs = Cursor::new(Vec::new());

        io::copy(r, &mut bs).await?;

        Ok(bs.into_inner())
    }

    /// Create a reader with specific compress algorithm.
    ///
    /// # Feature
    ///
    /// This function needs to enable feature `compress`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # use opendal::io_util::CompressAlgorithm;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file.gz");
    /// # o.write(vec![0; 4096]).await?;
    /// let r = o.decompress_reader_with(CompressAlgorithm::Gzip).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "compress")]
    pub async fn decompress_reader_with(&self, algo: CompressAlgorithm) -> Result<impl BytesRead> {
        let r = self.reader().await?;

        Ok(DecompressReader::new(r, algo))
    }

    /// Write bytes into object.
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// # use opendal::Scheme;
    /// use bytes::Bytes;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// let _ = o.write(vec![0; 4096]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write(&self, bs: impl Into<Vec<u8>>) -> Result<()> {
        if !validate_path(self.path(), ObjectMode::FILE) {
            return Err(new_other_object_error(
                Operation::Write,
                self.path(),
                anyhow!("Is a directory"),
            ));
        }

        let bs = bs.into();
        let op = OpWrite::new(bs.len() as u64);
        let r = Cursor::new(bs);
        let _ = self.acc.write(self.path(), op, Box::new(r)).await?;
        Ok(())
    }

    /// Write bytes into object.
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// # use opendal::Scheme;
    /// use bytes::Bytes;
    ///
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// let _ = o.blocking_write(vec![0; 4096])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_write(&self, bs: impl Into<Vec<u8>>) -> Result<()> {
        if !validate_path(self.path(), ObjectMode::FILE) {
            return Err(new_other_object_error(
                Operation::Write,
                self.path(),
                anyhow!("Is a directory"),
            ));
        }

        let bs = bs.into();
        let op = OpWrite::new(bs.len() as u64);
        let r = std::io::Cursor::new(bs);
        let _ = self.acc.blocking_write(self.path(), op, Box::new(r))?;
        Ok(())
    }

    /// Write data into object from a [`BytesRead`].
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// # use opendal::Scheme;
    /// use bytes::Bytes;
    /// use futures::io::Cursor;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// let r = Cursor::new(vec![0; 4096]);
    /// let _ = o.write_from(4096, r).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write_from(&self, size: u64, br: impl BytesRead + 'static) -> Result<()> {
        if !validate_path(self.path(), ObjectMode::FILE) {
            return Err(new_other_object_error(
                Operation::Write,
                self.path(),
                anyhow!("Is a directory"),
            ));
        }

        let _ = self
            .acc
            .write(self.path(), OpWrite::new(size), Box::new(br))
            .await?;
        Ok(())
    }

    /// Write data into object from a [`BlockingBytesRead`].
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// # use opendal::Scheme;
    /// use std::io::Cursor;
    ///
    /// use bytes::Bytes;
    ///
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/file");
    /// let r = Cursor::new(vec![0; 4096]);
    /// let _ = o.blocking_write_from(4096, r)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_write_from(
        &self,
        size: u64,
        br: impl BlockingBytesRead + 'static,
    ) -> Result<()> {
        if !validate_path(self.path(), ObjectMode::FILE) {
            return Err(new_other_object_error(
                Operation::Write,
                self.path(),
                anyhow!("Is a directory"),
            ));
        }

        let _ = self
            .acc
            .blocking_write(self.path(), OpWrite::new(size), Box::new(br))?;
        Ok(())
    }

    /// Delete object.
    ///
    /// # Notes
    ///
    /// - Delete not existing error won't return errors.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::Operator;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// op.object("test").delete().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(&self) -> Result<()> {
        self.acc.delete(self.path(), OpDelete::new()).await
    }

    /// Delete object.
    ///
    /// # Notes
    ///
    /// - Delete not existing error won't return errors.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::Operator;
    /// # use opendal::Scheme;
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// op.object("test").blocking_delete()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_delete(&self) -> Result<()> {
        self.acc.blocking_delete(self.path(), OpDelete::new())
    }

    /// List current dir object.
    ///
    /// This function will create a new [`ObjectStreamer`] handle to list objects.
    ///
    /// An error will be returned if object path doesn't end with `/`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::Operator;
    /// # use opendal::ObjectMode;
    /// # use futures::TryStreamExt;
    /// # use opendal::Scheme;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/dir/");
    /// let mut ds = o.list().await?;
    /// // ObjectStreamer implements `futures::Stream`
    /// while let Some(de) = ds.try_next().await? {
    ///     match de.mode() {
    ///         ObjectMode::FILE => {
    ///             println!("Handling file")
    ///         }
    ///         ObjectMode::DIR => {
    ///             println!("Handling dir like start a new list via meta.path()")
    ///         }
    ///         ObjectMode::Unknown => continue,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list(&self) -> Result<ObjectStreamer> {
        if !validate_path(self.path(), ObjectMode::DIR) {
            return Err(new_other_object_error(
                Operation::List,
                self.path(),
                anyhow!("Not a directory"),
            ));
        }

        self.acc.list(self.path(), OpList::new()).await
    }

    /// List current dir object.
    ///
    /// This function will create a new [`ObjectIterator`] handle to list objects.
    ///
    /// An error will be returned if object path doesn't end with `/`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::Operator;
    /// # use opendal::ObjectMode;
    /// # use opendal::Scheme;
    /// # fn main() -> Result<()> {
    /// use anyhow::anyhow;
    /// let op = Operator::from_env(Scheme::Memory)?;
    /// let o = op.object("path/to/dir/");
    /// let mut ds = o.blocking_list()?;
    /// while let Some(de) = ds.next() {
    ///     let de = de?;
    ///     match de.mode() {
    ///         ObjectMode::FILE => {
    ///             println!("Handling file")
    ///         }
    ///         ObjectMode::DIR => {
    ///             println!("Handling dir like start a new list via meta.path()")
    ///         }
    ///         ObjectMode::Unknown => continue,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_list(&self) -> Result<ObjectIterator> {
        if !validate_path(self.path(), ObjectMode::DIR) {
            return Err(new_other_object_error(
                Operation::List,
                self.path(),
                anyhow!("Not a directory"),
            ));
        }

        self.acc.blocking_list(self.path(), OpList::new())
    }

    /// Get current object's metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::Operator;
    /// # use opendal::Scheme;
    /// use std::io::ErrorKind;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// if let Err(e) = op.object("test").metadata().await {
    ///     if e.kind() == ErrorKind::NotFound {
    ///         println!("object not exist")
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn metadata(&self) -> Result<ObjectMetadata> {
        self.acc.stat(self.path(), OpStat::new()).await
    }

    /// Get current object's metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use opendal::services::memory;
    /// # use anyhow::Result;
    /// # use futures::io;
    /// # use opendal::Operator;
    /// # use opendal::Scheme;
    /// use std::io::ErrorKind;
    /// #
    /// # fn main() -> Result<()> {
    /// # let op = Operator::from_env(Scheme::Memory)?;
    /// if let Err(e) = op.object("test").blocking_metadata() {
    ///     if e.kind() == ErrorKind::NotFound {
    ///         println!("object not exist")
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn blocking_metadata(&self) -> Result<ObjectMetadata> {
        self.acc.blocking_stat(self.path(), OpStat::new())
    }

    /// Check if this object exists or not.
    ///
    /// # Example
    ///
    /// ```
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal::services::memory;
    /// use opendal::Operator;
    /// use opendal::Scheme;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let op = Operator::from_env(Scheme::Memory)?;
    ///     let _ = op.object("test").is_exist().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn is_exist(&self) -> Result<bool> {
        let r = self.metadata().await;
        match r {
            Ok(_) => Ok(true),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(false),
                _ => Err(err),
            },
        }
    }

    /// Check if this object exists or not.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anyhow::Result;
    /// use opendal::services::memory;
    /// use opendal::Operator;
    /// use opendal::Scheme;
    /// fn main() -> Result<()> {
    ///     let op = Operator::from_env(Scheme::Memory)?;
    ///     let _ = op.object("test").blocking_is_exist()?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn blocking_is_exist(&self) -> Result<bool> {
        let r = self.blocking_metadata();
        match r {
            Ok(_) => Ok(true),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(false),
                _ => Err(err),
            },
        }
    }

    /// Presign an operation for read.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal::services::memory;
    /// use opendal::Operator;
    /// use time::Duration;
    /// # use opendal::Scheme;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    /// #    let op = Operator::from_env(Scheme::Memory)?;
    ///     let signed_req = op.object("test").presign_read(Duration::hours(1))?;
    ///     let req = http::Request::builder()
    ///         .method(signed_req.method())
    ///         .uri(signed_req.uri())
    ///         .body(())?;
    ///
    /// #    Ok(())
    /// # }
    /// ```
    pub fn presign_read(&self, expire: Duration) -> Result<PresignedRequest> {
        let op = OpPresign::new(OpRead::new(..).into(), expire);

        self.acc.presign(self.path(), op)
    }

    /// Presign an operation for write.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal::services::memory;
    /// use opendal::Operator;
    /// use time::Duration;
    /// use opendal::Scheme;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    /// #    let op = Operator::from_env(Scheme::Memory)?;
    ///     let signed_req = op.object("test").presign_write(Duration::hours(1))?;
    ///     let req = http::Request::builder()
    ///         .method(signed_req.method())
    ///         .uri(signed_req.uri())
    ///         .body(())?;
    ///
    /// #    Ok(())
    /// # }
    /// ```
    pub fn presign_write(&self, expire: Duration) -> Result<PresignedRequest> {
        let op = OpPresign::new(OpWrite::new(0).into(), expire);

        self.acc.presign(self.path(), op)
    }

    /// Construct a multipart with existing upload id.
    pub fn to_multipart(&self, upload_id: &str) -> ObjectMultipart {
        ObjectMultipart::new(self.acc.clone(), &self.path, upload_id)
    }

    /// Create a new multipart for current path.
    pub async fn create_multipart(&self) -> Result<ObjectMultipart> {
        let upload_id = self
            .acc
            .create_multipart(self.path(), OpCreateMultipart::new())
            .await?;
        Ok(self.to_multipart(&upload_id))
    }
}

/// Metadata carries all object metadata.
///
/// # Notes
///
/// mode and content_length are required metadata that all services
/// should provide during `stat` operation. But in `list` operation,
/// a.k.a., `ObjectEntry`'s content length could be `None`.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ObjectMetadata {
    mode: ObjectMode,

    /// # NOTE
    ///
    /// - For `stat` operation, content_length is required to set.
    /// - For `list` operation, content_length could be None.
    content_length: Option<u64>,
    content_md5: Option<String>,
    last_modified: Option<OffsetDateTime>,
    etag: Option<String>,
}

impl ObjectMetadata {
    /// Create a new object metadata
    pub fn new(mode: ObjectMode) -> Self {
        Self {
            mode,

            content_length: None,
            content_md5: None,
            last_modified: None,
            etag: None,
        }
    }

    /// Object mode represent this object's mode.
    pub fn mode(&self) -> ObjectMode {
        self.mode
    }

    /// Set mode for object.
    pub fn set_mode(&mut self, mode: ObjectMode) -> &mut Self {
        self.mode = mode;
        self
    }

    /// Set mode for object.
    pub fn with_mode(mut self, mode: ObjectMode) -> Self {
        self.mode = mode;
        self
    }

    /// Content length of this object.
    ///
    /// `Content-Length` is defined by [RFC 7230](https://httpwg.org/specs/rfc7230.html#header.content-length)
    /// Refer to [MDN Content-Length](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Length) for more information.
    pub fn content_length(&self) -> u64 {
        self.content_length.unwrap_or_default()
    }

    /// Set content length of this object.
    pub fn set_content_length(&mut self, content_length: u64) -> &mut Self {
        self.content_length = Some(content_length);
        self
    }

    /// Set content length of this object.
    pub fn with_content_length(mut self, content_length: u64) -> Self {
        self.content_length = Some(content_length);
        self
    }

    /// Content MD5 of this object.
    ///
    /// Content MD5 is defined by [RFC 2616](http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html).
    /// And removed by [RFC 7231](https://www.rfc-editor.org/rfc/rfc7231).
    ///
    /// OpenDAL will try its best to set this value, but not guarantee this value is the md5 of content.
    pub fn content_md5(&self) -> Option<&str> {
        self.content_md5.as_deref()
    }

    /// Set content MD5 of this object.
    ///
    /// Content MD5 is defined by [RFC 2616](http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html).
    /// And removed by [RFC 7231](https://www.rfc-editor.org/rfc/rfc7231).
    pub fn set_content_md5(&mut self, content_md5: &str) -> &mut Self {
        self.content_md5 = Some(content_md5.to_string());
        self
    }

    /// Set content MD5 of this object.
    ///
    /// Content MD5 is defined by [RFC 2616](http://www.w3.org/Protocols/rfc2616/rfc2616-sec14.html).
    /// And removed by [RFC 7231](https://www.rfc-editor.org/rfc/rfc7231).
    pub fn with_content_md5(mut self, content_md5: &str) -> Self {
        self.content_md5 = Some(content_md5.to_string());
        self
    }

    /// Last modified of this object.
    ///
    /// `Last-Modified` is defined by [RFC 7232](https://httpwg.org/specs/rfc7232.html#header.last-modified)
    /// Refer to [MDN Last-Modified](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Last-Modified) for more information.
    ///
    /// OpenDAL parse the raw value into [`OffsetDateTime`] for convenient.
    pub fn last_modified(&self) -> Option<OffsetDateTime> {
        self.last_modified
    }

    /// Set Last modified of this object.
    ///
    /// `Last-Modified` is defined by [RFC 7232](https://httpwg.org/specs/rfc7232.html#header.last-modified)
    /// Refer to [MDN Last-Modified](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Last-Modified) for more information.
    pub fn set_last_modified(&mut self, last_modified: OffsetDateTime) -> &mut Self {
        self.last_modified = Some(last_modified);
        self
    }

    /// Set Last modified of this object.
    ///
    /// `Last-Modified` is defined by [RFC 7232](https://httpwg.org/specs/rfc7232.html#header.last-modified)
    /// Refer to [MDN Last-Modified](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Last-Modified) for more information.
    pub fn with_last_modified(mut self, last_modified: OffsetDateTime) -> Self {
        self.last_modified = Some(last_modified);
        self
    }

    /// ETag of this object.
    ///
    /// `ETag` is defined by [RFC 7232](https://httpwg.org/specs/rfc7232.html#header.etag)
    /// Refer to [MDN ETag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag) for more information.
    ///
    /// OpenDAL will return this value AS-IS like the following:
    ///
    /// - `"33a64df551425fcc55e4d42a148795d9f25f89d4"`
    /// - `W/"0815"`
    ///
    /// `"` is part of etag.
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    /// Set ETag of this object.
    ///
    /// `ETag` is defined by [RFC 7232](https://httpwg.org/specs/rfc7232.html#header.etag)
    /// Refer to [MDN ETag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag) for more information.
    ///
    /// OpenDAL will return this value AS-IS like the following:
    ///
    /// - `"33a64df551425fcc55e4d42a148795d9f25f89d4"`
    /// - `W/"0815"`
    ///
    /// `"` is part of etag, don't trim it before setting.
    pub fn set_etag(&mut self, etag: &str) -> &mut Self {
        self.etag = Some(etag.to_string());
        self
    }

    /// Set ETag of this object.
    ///
    /// `ETag` is defined by [RFC 7232](https://httpwg.org/specs/rfc7232.html#header.etag)
    /// Refer to [MDN ETag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/ETag) for more information.
    ///
    /// OpenDAL will return this value AS-IS like the following:
    ///
    /// - `"33a64df551425fcc55e4d42a148795d9f25f89d4"`
    /// - `W/"0815"`
    ///
    /// `"` is part of etag, don't trim it before setting.
    pub fn with_etag(mut self, etag: &str) -> Self {
        self.etag = Some(etag.to_string());
        self
    }
}

/// ObjectMode represents the corresponding object's mode.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ObjectMode {
    /// FILE means the object has data to read.
    FILE,
    /// DIR means the object can be listed.
    DIR,
    /// Unknown means we don't know what we can do on thi object.
    Unknown,
}

impl ObjectMode {
    /// Check if this object mode is FILE.
    pub fn is_file(self) -> bool {
        self == ObjectMode::FILE
    }
    /// Check if this object mode is DIR.
    pub fn is_dir(self) -> bool {
        self == ObjectMode::DIR
    }
}

impl Default for ObjectMode {
    fn default() -> Self {
        Self::Unknown
    }
}

impl Display for ObjectMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectMode::FILE => write!(f, "file"),
            ObjectMode::DIR => write!(f, "dir"),
            ObjectMode::Unknown => write!(f, "unknown"),
        }
    }
}

/// DirStream represents a stream of Dir.
pub trait ObjectStream: futures::Stream<Item = Result<ObjectEntry>> + Unpin + Send {}
impl<T> ObjectStream for T where T: futures::Stream<Item = Result<ObjectEntry>> + Unpin + Send {}

/// ObjectStreamer is a boxed dyn [`ObjectStream`]
pub type ObjectStreamer = Box<dyn ObjectStream>;

/// ObjectIterate represents an iterator of Dir.
pub trait ObjectIterate: Iterator<Item = Result<ObjectEntry>> {}
impl<T> ObjectIterate for T where T: Iterator<Item = Result<ObjectEntry>> {}

/// ObjectIterator is a boxed dyn [`ObjectIterate`]
pub type ObjectIterator = Box<dyn ObjectIterate>;

/// ObjectEntry is returned by [`ObjectStream`] during object list.
///
/// Users can check returning object entry's mode or convert into an object without overhead.
#[derive(Debug)]
pub struct ObjectEntry {
    acc: Arc<dyn Accessor>,
    path: String,
    meta: Arc<Mutex<ObjectMetadata>>,

    /// Complete means this object entry already carries all metadata
    /// that it could have. Don't call metadata anymore.
    ///
    /// # Safety
    ///
    /// It's safe to clone this value. Because complete will only transform
    /// into `true` and never change back.
    ///
    /// For the worse case, we cloned a `false` to new ObjectEntry. The cost
    /// is an extra stat.
    complete: AtomicBool,
}

impl Clone for ObjectEntry {
    fn clone(&self) -> Self {
        Self {
            acc: self.acc.clone(),
            path: self.path.clone(),
            meta: self.meta.clone(),
            // Read comments on this field.
            complete: self.complete.load(Ordering::Relaxed).into(),
        }
    }
}

impl ObjectEntry {
    /// Create a new object entry by its corresponding underlying storage.
    pub fn new(acc: Arc<dyn Accessor>, path: &str, meta: ObjectMetadata) -> ObjectEntry {
        debug_assert!(
            meta.mode.is_dir() == path.ends_with('/'),
            "mode {:?} not match with path {}",
            meta.mode,
            path
        );

        ObjectEntry {
            acc,
            path: path.to_string(),
            meta: Arc::new(Mutex::new(meta)),
            complete: AtomicBool::default(),
        }
    }

    /// Set accessor for this entry.
    pub fn set_accessor(&mut self, acc: Arc<dyn Accessor>) {
        self.acc = acc;
    }

    /// Complete means this object entry already carries all metadata
    /// that it could have. Don't call metadata anymore.
    pub fn set_complete(&mut self) -> &mut Self {
        self.complete.store(true, Ordering::Relaxed);
        self
    }

    /// Complete means this object entry already carries all metadata
    /// that it could have. Don't call metadata anymore.
    pub fn with_complete(mut self) -> Self {
        self.complete = AtomicBool::new(true);
        self
    }

    /// Convert [`ObjectEntry`] into [`Object`].
    ///
    /// This function is the same with already implemented `From` trait.
    /// This function will make our users happier to avoid writing
    /// generic type parameter
    pub fn into_object(self) -> Object {
        self.into()
    }

    /// Return this object entry's object mode.
    pub fn mode(&self) -> ObjectMode {
        self.meta.lock().expect("lock must succeed").mode
    }

    /// Return this object entry's id.
    ///
    /// The same with [`Object::id()`]
    pub fn id(&self) -> String {
        format!("{}{}", self.acc.metadata().root(), self.path)
    }

    /// Return this object entry's path.
    ///
    /// The same with [`Object::path()`]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Set path for this entry.
    pub fn set_path(&mut self, path: &str) {
        #[cfg(debug_assertions)]
        let mode = self.meta.lock().expect("lock must succeed").mode;
        debug_assert!(
            mode.is_dir() == path.ends_with('/'),
            "mode {mode:?} not match with path {path}",
        );

        self.path = path.to_string();
    }

    /// Return this object entry's name.
    ///
    /// The same with [`Object::name()`]
    pub fn name(&self) -> &str {
        get_basename(&self.path)
    }

    /// Fetch metadata about this object entry.
    pub async fn metadata(&self) -> ObjectMetadata {
        if !self.complete.load(Ordering::Relaxed) {
            // We will ignore all errors happened during inner metadata.
            if let Ok(meta) = self.acc.stat(self.path(), OpStat::new()).await {
                self.set_metadata(meta);
                self.complete.store(true, Ordering::Relaxed);
            }
        }

        self.meta.lock().expect("lock must succeed").clone()
    }

    /// Fetch metadata about this object entry.
    ///
    /// The same with [`Object::blocking_metadata()`]
    pub fn blocking_metadata(&self) -> ObjectMetadata {
        if !self.complete.load(Ordering::Relaxed) {
            // We will ignore all errors happened during inner metadata.
            if let Ok(meta) = self.acc.blocking_stat(self.path(), OpStat::new()) {
                self.set_metadata(meta);
                self.complete.store(true, Ordering::Relaxed);
            }
        }

        self.meta.lock().expect("lock must succeed").clone()
    }

    /// Update ObjectEntry's metadata by setting new one.
    pub fn set_metadata(&self, meta: ObjectMetadata) -> &Self {
        let mut guard = self.meta.lock().expect("lock must succeed");
        *guard = meta;
        self
    }

    /// The size of `ObjectEntry`'s corresponding object
    ///
    /// `content_length` is a prefetched metadata field in `ObjectEntry`.
    pub async fn content_length(&self) -> u64 {
        if let Some(v) = self.meta.lock().expect("lock must succeed").content_length {
            return v;
        }

        self.metadata().await.content_length.unwrap_or_default()
    }

    /// The MD5 message digest of `ObjectEntry`'s corresponding object
    ///
    /// `content_md5` is a prefetched metadata field in `ObjectEntry`
    ///
    /// It doesn't mean this metadata field of object doesn't exist if `content_md5` is `None`.
    /// Then you have to call `ObjectEntry::metadata()` to get the metadata you want.
    pub async fn content_md5(&self) -> Option<String> {
        if let Some(v) = self
            .meta
            .lock()
            .expect("lock must succeed")
            .content_md5
            .clone()
        {
            return Some(v);
        }

        self.metadata().await.content_md5
    }

    /// The last modified UTC datetime of `ObjectEntry`'s corresponding object
    ///
    /// `last_modified` is a prefetched metadata field in `ObjectEntry`
    ///
    /// It doesn't mean this metadata field of object doesn't exist if `last_modified` is `None`.
    /// Then you have to call `ObjectEntry::metadata()` to get the metadata you want.
    pub async fn last_modified(&self) -> Option<OffsetDateTime> {
        if let Some(v) = self.meta.lock().expect("lock must succeed").last_modified {
            return Some(v);
        }

        self.metadata().await.last_modified
    }

    /// The ETag string of `ObjectEntry`'s corresponding object
    ///
    /// `etag` is a prefetched metadata field in `ObjectEntry`.
    ///
    /// It doesn't mean this metadata field of object doesn't exist if `etag` is `None`.
    /// Then you have to call `ObjectEntry::metadata()` to get the metadata you want.
    pub async fn etag(&self) -> Option<String> {
        if let Some(v) = self.meta.lock().expect("lock must succeed").etag.clone() {
            return Some(v);
        }

        self.metadata().await.etag
    }
}

/// ObjectEntry can convert into object without overhead.
impl From<ObjectEntry> for Object {
    fn from(d: ObjectEntry) -> Self {
        Object::new(d.acc, &d.path)
    }
}

/// EmptyObjectStreamer that always return None.
pub(crate) struct EmptyObjectStreamer;

impl futures::Stream for EmptyObjectStreamer {
    type Item = Result<ObjectEntry>;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(None)
    }
}

/// EmptyObjectIterator that always return None.
pub(crate) struct EmptyObjectIterator;

impl Iterator for EmptyObjectIterator {
    type Item = Result<ObjectEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
