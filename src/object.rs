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
use std::sync::Arc;
use std::time::SystemTime;

use futures::io;
use futures::io::Cursor;
use futures::AsyncWriteExt;

use crate::io::BytesRead;
use crate::ops::OpDelete;
use crate::ops::OpRead;
use crate::ops::OpStat;
use crate::ops::OpWrite;
use crate::Accessor;
use crate::BytesWrite;

/// Handler for all object related operations.
#[derive(Clone, Debug)]
pub struct Object {
    acc: Arc<dyn Accessor>,
    meta: Metadata,
}

impl Object {
    /// Creates a new Object.
    pub fn new(acc: Arc<dyn Accessor>, path: &str) -> Self {
        Self {
            acc,
            meta: Metadata {
                path: path.to_string(),
                ..Default::default()
            },
        }
    }

    pub(crate) fn accessor(&self) -> Arc<dyn Accessor> {
        self.acc.clone()
    }

    /// Read the whole object into a bytes.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`Object::stream`] or [`Object::reader`]
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::new(memory::Backend::build().finish().await?);
    /// # let o = op.object("path/to/file");
    /// # o.write(&vec![0; 4096]).await?;
    /// let bs = o.read().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn read(&self) -> Result<Vec<u8>> {
        self.range_read(..).await
    }

    /// Read the specified range of object into a bytes.
    ///
    /// This function will allocate a new bytes internally. For more precise memory control or
    /// reading data lazily, please use [`Object::stream`] or [`Object::range_reader`]
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::new(memory::Backend::build().finish().await?);
    /// # let o = op.object("path/to/file");
    /// # o.write(&vec![0; 4096]).await?;
    /// let bs = o.range_read(1024..2048).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn range_read(&self, range: impl RangeBounds<u64>) -> Result<Vec<u8>> {
        let op = OpRead::new(self.meta.path(), range);
        let s = self.acc.read(&op).await?;

        let mut bs = Cursor::new(Vec::new());

        io::copy(s, &mut bs).await?;

        Ok(bs.into_inner())
    }

    /// Create a new reader which can read the whole object.
    ///
    /// This function adopt a zero cost conversion from [`BytesStream`][crate::BytesStream] to [`BytesRead`][crate::BytesRead].
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::new(memory::Backend::build().finish().await?);
    /// # let o = op.object("path/to/file");
    /// # o.write(&vec![0; 4096]).await?;
    /// let r = o.reader().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn reader(&self) -> Result<impl BytesRead> {
        self.range_reader(..).await
    }

    /// Create a new reader which can read the whole object.
    ///
    /// This function adopt a zero cost conversion from [`BytesStream`][crate::BytesStream] to [`BytesRead`][crate::BytesRead].
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::TryStreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::new(memory::Backend::build().finish().await?);
    /// # let o = op.object("path/to/file");
    /// # o.write(&vec![0; 4096]).await?;
    /// let r = o.range_reader(1024..2048).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn range_reader(&self, range: impl RangeBounds<u64>) -> Result<impl BytesRead> {
        let op = OpRead::new(self.meta.path(), range);
        Ok(self.acc.read(&op).await?)
    }

    /// Write bytes into object.
    ///
    /// # Notes
    ///
    /// - Write will make sure all bytes has been written, or an error will be returned.
    /// - Input bytes will be sent directly without extra copy.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// use bytes::Bytes;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::new(memory::Backend::build().finish().await?);
    /// let o = op.object("path/to/file");
    /// let _ = o.write(vec![0; 4096]).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write(&self, bs: impl AsRef<[u8]>) -> Result<()> {
        let op = OpWrite::new(self.meta.path(), bs.as_ref().len() as u64);
        let mut s = self.acc.write(&op).await?;

        s.write_all(bs.as_ref()).await?;
        s.close().await?;

        Ok(())
    }

    /// Create a new writer which can write data into the object.
    ///
    /// # Examples
    ///
    /// ```
    /// # use opendal::services::memory;
    /// # use std::io::Result;
    /// # use opendal::Operator;
    /// # use futures::StreamExt;
    /// # use futures::SinkExt;
    /// use bytes::Bytes;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # use futures::AsyncWriteExt;
    /// let op = Operator::new(memory::Backend::build().finish().await?);
    /// let o = op.object("path/to/file");
    /// let mut w = o.writer(4096).await?;
    /// w.write(&[1; 4096]);
    /// w.close();
    /// # Ok(())
    /// # }
    /// ```
    pub async fn writer(&self, size: u64) -> Result<impl BytesWrite> {
        let op = OpWrite::new(self.meta.path(), size);
        let s = self.acc.write(&op).await?;

        Ok(s)
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
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::new(memory::Backend::build().finish().await?);
    /// op.object("test").delete().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(&self) -> Result<()> {
        let op = &OpDelete::new(self.meta.path());

        self.acc.delete(op).await
    }

    pub(crate) fn metadata_ref(&self) -> &Metadata {
        &self.meta
    }

    pub(crate) fn metadata_mut(&mut self) -> &mut Metadata {
        &mut self.meta
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
    /// use std::io::ErrorKind;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<()> {
    /// # let op = Operator::new(memory::Backend::build().finish().await?);
    /// if let Err(e) = op.object("test").metadata().await {
    ///     if e.kind() == ErrorKind::NotFound {
    ///         println!("object not exist")
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn metadata(&self) -> Result<Metadata> {
        let op = &OpStat::new(self.meta.path());

        self.acc.stat(op).await
    }

    /// Use local cached metadata if possible.
    ///
    /// # Example
    ///
    /// ```
    /// use opendal::services::memory;
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal::Operator;
    ///
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let op = Operator::new(memory::Backend::build().finish().await?);
    ///     let mut o = op.object("test");
    ///
    ///     o.metadata_cached().await;
    ///     // The second call to metadata_cached will have no cost.
    ///     o.metadata_cached().await;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn metadata_cached(&mut self) -> Result<&Metadata> {
        if self.meta.complete() {
            return Ok(&self.meta);
        }

        let op = &OpStat::new(self.meta.path());
        self.meta = self.acc.stat(op).await?;

        Ok(&self.meta)
    }

    /// Check if this object exist or not.
    ///
    /// # Example
    ///
    /// ```
    /// use opendal::services::memory;
    /// use anyhow::Result;
    /// use futures::io;
    /// use opendal::Operator;
    ///
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let op = Operator::new(memory::Backend::build().finish().await?);
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
}

/// Metadata carries all object metadata.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    complete: bool,

    path: String,
    mode: Option<ObjectMode>,

    content_length: Option<u64>,
    content_md5: Option<String>,
    last_modified: Option<SystemTime>,
}

impl Metadata {
    pub fn complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn set_complete(&mut self) -> &mut Self {
        self.complete = true;
        self
    }

    /// Returns object path that relative to corresponding backend's root.
    pub fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn set_path(&mut self, path: &str) -> &mut Self {
        self.path = path.to_string();
        self
    }

    /// Object mode represent this object' mode.
    pub fn mode(&self) -> ObjectMode {
        debug_assert!(self.mode.is_some(), "mode must exist");

        self.mode.unwrap_or_default()
    }

    pub(crate) fn set_mode(&mut self, mode: ObjectMode) -> &mut Self {
        self.mode = Some(mode);
        self
    }

    /// Content length of this object
    pub fn content_length(&self) -> u64 {
        debug_assert!(self.content_length.is_some(), "content length must exist");

        self.content_length.unwrap_or_default()
    }

    pub(crate) fn set_content_length(&mut self, content_length: u64) -> &mut Self {
        self.content_length = Some(content_length);
        self
    }

    /// Content MD5 of this object.
    pub fn content_md5(&self) -> Option<String> {
        self.content_md5.clone()
    }

    pub(crate) fn set_content_md5(&mut self, content_md5: &str) -> &mut Self {
        self.content_md5 = Some(content_md5.to_string());
        self
    }

    /// Last modified of this object.
    pub fn last_modified(&self) -> Option<SystemTime> {
        self.last_modified
    }

    pub(crate) fn set_last_modified(&mut self, last_modified: SystemTime) -> &mut Self {
        self.last_modified = Some(last_modified);
        self
    }
}

/// ObjectMode represents the corresponding object's mode.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ObjectMode {
    /// FILE means the object has data to read.
    FILE,
    /// DIR means the object can be listed.
    DIR,
    /// Unknown means we don't know what we can do on thi object.
    Unknown,
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

/// ObjectStream represents a stream of object.
pub trait ObjectStream: futures::Stream<Item = Result<Object>> + Unpin + Send {}
impl<T> ObjectStream for T where T: futures::Stream<Item = Result<Object>> + Unpin + Send {}

/// ObjectStreamer is a boxed dyn [`ObjectStream`]
pub type ObjectStreamer = Box<dyn ObjectStream>;
