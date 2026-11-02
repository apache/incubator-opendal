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

#[macro_use]
extern crate napi_derive;

use std::collections::HashMap;
use std::fmt::Display;
use std::io::Read;
use std::time::Duration;

use futures::AsyncReadExt;
use futures::TryStreamExt;
use napi::bindgen_prelude::*;
use opendal::options::{
    DeleteOptions, ListOptions, ReadOptions, ReaderOptions, StatOptions, WriteOptions,
};

use crate::layer::Layer;

mod capability;
mod layer;
mod options;

#[napi]
pub struct Operator {
    async_op: opendal::Operator,
    blocking_op: opendal::blocking::Operator,
}

#[napi]
impl Operator {
    /// @see For the full list of scheme, see https://docs.rs/opendal/latest/opendal/services/index.html
    /// And the options,
    /// please refer to the documentation of the corresponding service for the corresponding parameters.
    /// Note that the current options key is snake_case.
    #[napi(constructor, async_runtime)]
    pub fn new(scheme: String, options: Option<HashMap<String, String>>) -> Result<Self> {
        let options = options.unwrap_or_default();

        let async_op = opendal::Operator::via_iter(scheme, options).map_err(format_napi_error)?;

        let blocking_op =
            opendal::blocking::Operator::new(async_op.clone()).map_err(format_napi_error)?;

        Ok(Operator {
            async_op,
            blocking_op,
        })
    }

    /// Create a new operator from a URI string.
    ///
    /// The URI encodes the scheme and configuration in a single string, e.g.
    /// `memory://localhost/` or `s3://bucket/path?region=us-east-1`.
    ///
    /// Optional extra key-value options can be passed to override or supplement
    /// the values encoded in the URI.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const op = Operator.fromUri("memory://localhost/");
    /// const op2 = Operator.fromUri("s3://my-bucket/", { region: "us-east-1" });
    /// ```
    #[napi(factory, async_runtime)]
    pub fn from_uri(uri: String, options: Option<HashMap<String, String>>) -> Result<Self> {
        let options = options.unwrap_or_default();
        let async_op = if options.is_empty() {
            opendal::Operator::from_uri(uri).map_err(format_napi_error)?
        } else {
            opendal::Operator::from_uri((uri.as_str(), options)).map_err(format_napi_error)?
        };

        let blocking_op =
            opendal::blocking::Operator::new(async_op.clone()).map_err(format_napi_error)?;

        Ok(Operator {
            async_op,
            blocking_op,
        })
    }

    /// Get current operator(service)'s full capability.
    #[napi]
    pub fn capability(&self) -> Result<capability::Capability> {
        Ok(capability::Capability::new(
            self.async_op.info().capability(),
        ))
    }

    /// Get current path's metadata **without cache** directly.
    ///
    /// ### Notes
    /// Use stat if you:
    ///
    /// - Want to detect the outside changes of a path.
    /// - Don’t want to read from cached metadata.
    ///
    /// You may want to use `metadata` if you are working with entries returned by `Lister`. It’s highly possible that metadata you want has already been cached.
    ///
    /// ### Example
    /// ```javascript
    /// const meta = await op.stat("test");
    /// if (meta.isDir) {
    ///   // do something
    /// }
    /// ```
    #[napi]
    pub async fn stat(
        &self,
        path: String,
        options: Option<options::StatOptions>,
    ) -> Result<Metadata> {
        let options = options.map_or_else(StatOptions::default, StatOptions::from);
        let meta = self
            .async_op
            .stat_options(&path, options)
            .await
            .map_err(format_napi_error)?;

        Ok(Metadata(meta))
    }

    /// Get current path's metadata **without cache** directly and synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// const meta = op.statSync("test");
    /// if (meta.isDir) {
    ///   // do something
    /// }
    /// ```
    #[napi]
    pub fn stat_sync(
        &self,
        path: String,
        options: Option<options::StatOptions>,
    ) -> Result<Metadata> {
        let options = options.map_or_else(StatOptions::default, StatOptions::from);
        let meta = self
            .blocking_op
            .stat_options(&path, options)
            .map_err(format_napi_error)?;

        Ok(Metadata(meta))
    }

    /// Check if this operator can work correctly.
    ///
    /// We will send a `list` request to the given path and return any errors we met.
    ///
    /// ### Example
    /// ```javascript
    /// await op.check();
    /// ```
    #[napi]
    pub async fn check(&self) -> Result<()> {
        self.async_op.check().await.map_err(format_napi_error)
    }

    /// Check the op synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.checkSync();
    /// ```
    #[napi]
    pub fn check_sync(&self) -> Result<()> {
        self.blocking_op.check().map_err(format_napi_error)
    }

    /// Check if this path exists or not.
    ///
    /// ### Example
    /// ```javascript
    /// await op.isExist("test");
    /// ```
    #[napi]
    pub async fn exists(&self, path: String) -> Result<bool> {
        self.async_op.exists(&path).await.map_err(format_napi_error)
    }

    /// Check if this path exists or not synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.isExistSync("test");
    /// ```
    #[napi]
    pub fn exists_sync(&self, path: String) -> Result<bool> {
        self.blocking_op.exists(&path).map_err(format_napi_error)
    }

    /// Create dir with a given path.
    ///
    /// ### Example
    /// ```javascript
    /// await op.createDir("path/to/dir/");
    /// ```
    #[napi]
    pub async fn create_dir(&self, path: String) -> Result<()> {
        self.async_op
            .create_dir(&path)
            .await
            .map_err(format_napi_error)
    }

    /// Create dir with a given path synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.createDirSync("path/to/dir/");
    /// ```
    #[napi]
    pub fn create_dir_sync(&self, path: String) -> Result<()> {
        self.blocking_op
            .create_dir(&path)
            .map_err(format_napi_error)
    }

    /// Read the whole path into a buffer.
    ///
    /// ### Example
    /// ```javascript
    /// const buf = await op.read("path/to/file");
    /// ```
    #[napi]
    pub async fn read(
        &self,
        path: String,
        options: Option<options::ReadOptions>,
    ) -> Result<Buffer> {
        let options = options.map_or_else(ReadOptions::default, ReadOptions::from);
        let res = self
            .async_op
            .read_options(&path, options)
            .await
            .map_err(format_napi_error)?
            .to_vec();
        Ok(res.into())
    }

    /// Create a reader to read the given path.
    ///
    /// It could be used to read large file in a streaming way.
    #[napi]
    pub async fn reader(
        &self,
        path: String,
        options: Option<options::ReaderOptions>,
    ) -> Result<Reader> {
        let options = options.map_or_else(ReaderOptions::default, ReaderOptions::from);
        let r = self
            .async_op
            .reader_options(&path, options)
            .await
            .map_err(format_napi_error)?;
        Ok(Reader {
            inner: r
                .into_futures_async_read(std::ops::RangeFull)
                .await
                .map_err(format_napi_error)?,
        })
    }

    /// Read the whole path into a buffer synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// const buf = op.readSync("path/to/file");
    /// ```
    #[napi]
    pub fn read_sync(&self, path: String, options: Option<options::ReadOptions>) -> Result<Buffer> {
        let options = options.map_or_else(ReadOptions::default, ReadOptions::from);
        let res = self
            .blocking_op
            .read_options(&path, options)
            .map_err(format_napi_error)?
            .to_vec();
        Ok(res.into())
    }

    /// Create a reader to read the given path synchronously.
    ///
    /// It could be used to read large file in a streaming way.
    #[napi]
    pub fn reader_sync(
        &self,
        path: String,
        options: Option<options::ReaderOptions>,
    ) -> Result<BlockingReader> {
        let options = options.map_or_else(ReaderOptions::default, ReaderOptions::from);
        let r = self
            .blocking_op
            .reader_options(&path, options)
            .map_err(format_napi_error)?;
        Ok(BlockingReader {
            inner: r.into_std_read(..).map_err(format_napi_error)?,
        })
    }

    //noinspection DuplicatedCode
    /// Write bytes into a path.
    ///
    /// ### Example
    /// ```javascript
    /// await op.write("path/to/file", Buffer.from("hello world"));
    /// // or
    /// await op.write("path/to/file", "hello world");
    /// // or
    /// await op.write("path/to/file", Buffer.from("hello world"), { contentType: "text/plain" });
    /// ```
    #[napi]
    pub async fn write(
        &self,
        path: String,
        content: Either<Buffer, String>,
        options: Option<options::WriteOptions>,
    ) -> Result<Metadata> {
        let c = match content {
            Either::A(buf) => buf.as_ref().to_owned(),
            Either::B(s) => s.into_bytes(),
        };
        let options = options.map_or_else(WriteOptions::default, WriteOptions::from);
        let metadata = self
            .async_op
            .write_options(&path, c, options)
            .await
            .map_err(format_napi_error)?;
        Ok(Metadata(metadata))
    }

    //noinspection DuplicatedCode
    /// Write multiple bytes into a path.
    ///
    /// It could be used to write large file in a streaming way.
    #[napi]
    pub async fn writer(
        &self,
        path: String,
        options: Option<options::WriteOptions>,
    ) -> Result<Writer> {
        let options = options.unwrap_or_default();
        let writer = self
            .async_op
            .writer_options(&path, options.into())
            .await
            .map_err(format_napi_error)?;
        Ok(Writer(writer))
    }

    /// Write multiple bytes into a path synchronously.
    ///
    /// It could be used to write large file in a streaming way.
    #[napi]
    pub fn writer_sync(
        &self,
        path: String,
        options: Option<options::WriteOptions>,
    ) -> Result<BlockingWriter> {
        let options = options.unwrap_or_default();
        let writer = self
            .blocking_op
            .writer_options(&path, options.into())
            .map_err(format_napi_error)?;
        Ok(BlockingWriter(writer))
    }

    //noinspection DuplicatedCode
    /// Write bytes into a path synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.writeSync("path/to/file", Buffer.from("hello world"));
    /// // or
    /// op.writeSync("path/to/file", "hello world");
    /// // or
    /// op.writeSync("path/to/file", Buffer.from("hello world"), { contentType: "text/plain" });
    /// ```
    #[napi]
    pub fn write_sync(
        &self,
        path: String,
        content: Either<Buffer, String>,
        options: Option<options::WriteOptions>,
    ) -> Result<Metadata> {
        let c = match content {
            Either::A(buf) => buf.as_ref().to_owned(),
            Either::B(s) => s.into_bytes(),
        };
        let options = options.map_or_else(WriteOptions::default, WriteOptions::from);
        let metadata = self
            .blocking_op
            .write_options(&path, c, options)
            .map_err(format_napi_error)?;
        Ok(Metadata(metadata))
    }

    /// Copy file according to given `from` and `to` path.
    ///
    /// ### Example
    /// ```javascript
    /// await op.copy("path/to/file", "path/to/dest");
    /// ```
    #[napi]
    pub async fn copy(&self, from: String, to: String) -> Result<()> {
        self.async_op
            .copy(&from, &to)
            .await
            .map(|_| ())
            .map_err(format_napi_error)
    }

    /// Copy file according to given `from` and `to` path synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.copySync("path/to/file", "path/to/dest");
    /// ```
    #[napi]
    pub fn copy_sync(&self, from: String, to: String) -> Result<()> {
        self.blocking_op
            .copy(&from, &to)
            .map(|_| ())
            .map_err(format_napi_error)
    }

    /// Rename file according to given `from` and `to` path.
    ///
    /// It's similar to `mv` command.
    ///
    /// ### Example
    /// ```javascript
    /// await op.rename("path/to/file", "path/to/dest");
    /// ```
    #[napi]
    pub async fn rename(&self, from: String, to: String) -> Result<()> {
        self.async_op
            .rename(&from, &to)
            .await
            .map_err(format_napi_error)
    }

    /// Rename file according to given `from` and `to` path synchronously.
    ///
    /// It's similar to `mv` command.
    ///
    /// ### Example
    /// ```javascript
    /// op.renameSync("path/to/file", "path/to/dest");
    /// ```
    #[napi]
    pub fn rename_sync(&self, from: String, to: String) -> Result<()> {
        self.blocking_op
            .rename(&from, &to)
            .map_err(format_napi_error)
    }

    /// Delete the given path.
    ///
    /// ### Notes
    /// Delete not existing error won’t return errors.
    ///
    /// ### Example
    /// ```javascript
    /// await op.delete("test");
    /// ```
    #[napi]
    pub async fn delete(
        &self,
        path: String,
        options: Option<options::DeleteOptions>,
    ) -> Result<()> {
        let options = options.map_or_else(DeleteOptions::default, DeleteOptions::from);
        self.async_op
            .delete_options(&path, options)
            .await
            .map_err(format_napi_error)
    }

    /// Delete the given path synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.deleteSync("test");
    /// ```
    #[napi]
    pub fn delete_sync(&self, path: String, options: Option<options::DeleteOptions>) -> Result<()> {
        let options = options.map_or_else(DeleteOptions::default, DeleteOptions::from);
        self.blocking_op
            .delete_options(&path, options)
            .map_err(format_napi_error)
    }

    /// Remove given paths.
    ///
    /// ### Notes
    /// If underlying services support delete in batch, we will use batch delete instead.
    ///
    /// ### Examples
    /// ```javascript
    /// await op.remove(["abc", "def"]);
    /// ```
    #[napi]
    pub async fn remove(&self, paths: Vec<String>) -> Result<()> {
        self.async_op
            .delete_iter(paths)
            .await
            .map_err(format_napi_error)
    }

    /// Remove given paths.
    ///
    /// ### Notes
    /// If underlying services support delete in batch, we will use batch delete instead.
    ///
    /// ### Examples
    /// ```javascript
    /// op.removeSync(["abc", "def"]);
    /// ```
    #[napi]
    pub fn remove_sync(&self, paths: Vec<String>) -> Result<()> {
        self.blocking_op
            .delete_iter(paths)
            .map_err(format_napi_error)
    }

    /// Remove the path and all nested dirs and files recursively.
    ///
    /// ### Notes
    /// If underlying services support delete in batch, we will use batch delete instead.
    ///
    /// ### Examples
    /// ```javascript
    /// await op.removeAll("path/to/dir/");
    /// ```
    #[napi]
    pub async fn remove_all(&self, path: String) -> Result<()> {
        self.async_op
            .delete_with(&path)
            .recursive(true)
            .await
            .map_err(format_napi_error)
    }

    /// Remove the path and all nested dirs and files recursively.
    ///
    /// ### Notes
    /// If underlying services support delete in batch, we will use batch delete instead.
    ///
    /// ### Examples
    /// ```javascript
    /// op.removeAllSync("path/to/dir/");
    /// ```
    #[napi]
    pub fn remove_all_sync(&self, path: String) -> Result<()> {
        use opendal::options::DeleteOptions;
        self.blocking_op
            .delete_options(
                &path,
                DeleteOptions {
                    recursive: true,
                    ..Default::default()
                },
            )
            .map_err(format_napi_error)
    }

    /// List the given path.
    ///
    /// This function will return an array of entries.
    ///
    /// An error will be returned if given path doesn't end with `/`.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const list = await op.list("path/to/dir/");
    /// for (let entry of list) {
    ///   let meta = await op.stat(entry.path);
    ///   if (meta.isFile) {
    ///     // do something
    ///   }
    /// }
    /// ```
    ///
    /// #### List recursively
    ///
    /// With `recursive` option, you can list recursively.
    ///
    /// ```javascript
    /// const list = await op.list("path/to/dir/", { recursive: true });
    /// for (let entry of list) {
    ///   let meta = await op.stat(entry.path);
    ///   if (meta.isFile) {
    ///     // do something
    ///   }
    /// }
    /// ```
    #[napi]
    pub async fn list(
        &self,
        path: String,
        options: Option<options::ListOptions>,
    ) -> Result<Vec<Entry>> {
        let options = options.map_or(ListOptions::default(), ListOptions::from);
        let l = self
            .async_op
            .list_options(&path, options)
            .await
            .map_err(format_napi_error)?;

        Ok(l.into_iter().map(Entry).collect())
    }

    /// List the given path synchronously.
    ///
    /// This function will return an array of entries.
    ///
    /// An error will be returned if given path doesn't end with `/`.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const list = op.listSync("path/to/dir/");
    /// for (let entry of list) {
    ///   let meta = op.statSync(entry.path);
    ///   if (meta.isFile) {
    ///     // do something
    ///   }
    /// }
    /// ```
    ///
    /// #### List recursively
    ///
    /// With `recursive` option, you can list recursively.
    ///
    /// ```javascript
    /// const list = op.listSync("path/to/dir/", { recursive: true });
    /// for (let entry of list) {
    ///   let path = entry.path();
    ///   let meta = entry.metadata();
    ///   if (meta.isFile) {
    ///     // do something
    ///   }
    /// }
    /// ```
    #[napi]
    pub fn list_sync(
        &self,
        path: String,
        options: Option<options::ListOptions>,
    ) -> Result<Vec<Entry>> {
        let options = options.map_or(ListOptions::default(), ListOptions::from);
        let l = self
            .blocking_op
            .list_options(&path, options)
            .map_err(format_napi_error)?;

        Ok(l.into_iter().map(Entry).collect())
    }

    /// Create a lister to list entries at given path.
    ///
    /// This function returns a Lister that can be used to iterate over entries
    /// in a streaming manner, which is more memory-efficient for large directories.
    ///
    /// An error will be returned if given path doesn't end with `/`.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const lister = await op.lister("path/to/dir/");
    /// let entry;
    /// while ((entry = await lister.next()) !== null) {
    ///   console.log(entry.path());
    /// }
    /// ```
    ///
    /// #### List recursively
    ///
    /// With `recursive` option, you can list recursively.
    ///
    /// ```javascript
    /// const lister = await op.lister("path/to/dir/", { recursive: true });
    /// let entry;
    /// while ((entry = await lister.next()) !== null) {
    ///   console.log(entry.path());
    /// }
    /// ```
    #[napi]
    pub async fn lister(
        &self,
        path: String,
        options: Option<options::ListOptions>,
    ) -> Result<Lister> {
        let options = options.map_or(ListOptions::default(), ListOptions::from);
        let l = self
            .async_op
            .lister_options(&path, options)
            .await
            .map_err(format_napi_error)?;

        Ok(Lister(l))
    }

    /// Create a lister to list entries at given path synchronously.
    ///
    /// This function returns a BlockingLister that can be used to iterate over entries
    /// in a streaming manner, which is more memory-efficient for large directories.
    ///
    /// An error will be returned if given path doesn't end with `/`.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const lister = op.listerSync("path/to/dir/");
    /// let entry;
    /// while ((entry = lister.next()) !== null) {
    ///   console.log(entry.path());
    /// }
    /// ```
    ///
    /// #### List recursively
    ///
    /// With `recursive` option, you can list recursively.
    ///
    /// ```javascript
    /// const lister = op.listerSync("path/to/dir/", { recursive: true });
    /// let entry;
    /// while ((entry = lister.next()) !== null) {
    ///   console.log(entry.path());
    /// }
    /// ```
    #[napi]
    pub fn lister_sync(
        &self,
        path: String,
        options: Option<options::ListOptions>,
    ) -> Result<BlockingLister> {
        let options = options.map_or(ListOptions::default(), ListOptions::from);
        let l = self
            .blocking_op
            .lister_options(&path, options)
            .map_err(format_napi_error)?;

        Ok(BlockingLister(l))
    }

    /// Get a presigned request for read.
    ///
    /// Unit of `expires` is seconds.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const req = await op.presignRead(path, parseInt(expires));
    ///
    /// console.log("method: ", req.method);
    /// console.log("url: ", req.url);
    /// console.log("headers: ", req.headers);
    /// ```
    #[napi]
    pub async fn presign_read(&self, path: String, expires: u32) -> Result<PresignedRequest> {
        let res = self
            .async_op
            .presign_read(&path, Duration::from_secs(expires as u64))
            .await
            .map_err(format_napi_error)?;
        Ok(PresignedRequest::new(res))
    }

    /// Get a presigned request for `write`.
    ///
    /// Unit of `expires` is seconds.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const req = await op.presignWrite(path, parseInt(expires));
    ///
    /// console.log("method: ", req.method);
    /// console.log("url: ", req.url);
    /// console.log("headers: ", req.headers);
    /// ```
    #[napi]
    pub async fn presign_write(&self, path: String, expires: u32) -> Result<PresignedRequest> {
        let res = self
            .async_op
            .presign_write(&path, Duration::from_secs(expires as u64))
            .await
            .map_err(format_napi_error)?;
        Ok(PresignedRequest::new(res))
    }

    /// Get a presigned request for stat.
    ///
    /// Unit of `expires` is seconds.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const req = await op.presignStat(path, parseInt(expires));
    ///
    /// console.log("method: ", req.method);
    /// console.log("url: ", req.url);
    /// console.log("headers: ", req.headers);
    /// ```
    #[napi]
    pub async fn presign_stat(&self, path: String, expires: u32) -> Result<PresignedRequest> {
        let res = self
            .async_op
            .presign_stat(&path, Duration::from_secs(expires as u64))
            .await
            .map_err(format_napi_error)?;
        Ok(PresignedRequest::new(res))
    }

    /// Get a presigned request for delete.
    ///
    /// Unit of `expires` is seconds.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const req = await op.presignDelete(path, parseInt(expires));
    ///
    /// console.log("method: ", req.method);
    /// console.log("url: ", req.url);
    /// console.log("headers: ", req.headers);
    /// ```
    #[napi]
    pub async fn presign_delete(&self, path: String, expires: u32) -> Result<PresignedRequest> {
        let res = self
            .async_op
            .presign_delete(&path, Duration::from_secs(expires as u64))
            .await
            .map_err(format_napi_error)?;
        Ok(PresignedRequest::new(res))
    }
}

/// Entry returned by Lister or BlockingLister to represent a path, and it's a relative metadata.
#[napi]
pub struct Entry(opendal::Entry);

#[napi]
impl Entry {
    /// Return the path of this entry.
    #[napi]
    pub fn path(&self) -> String {
        self.0.path().to_string()
    }

    /// Return the name of this entry.
    #[napi]
    pub fn name(&self) -> String {
        self.0.name().to_string()
    }

    /// Return the metadata of this entry.
    #[napi]
    pub fn metadata(&self) -> Metadata {
        Metadata(self.0.metadata().clone())
    }
}

#[napi]
pub enum EntryMode {
    /// FILE means the path has data to read.
    FILE,
    /// DIR means the path can be listed.
    DIR,
    /// Unknown means we don't know what we can do on this path.
    Unknown,
}

impl From<opendal::EntryMode> for EntryMode {
    fn from(mode: opendal::EntryMode) -> Self {
        match mode {
            opendal::EntryMode::FILE => EntryMode::FILE,
            opendal::EntryMode::DIR => EntryMode::DIR,
            opendal::EntryMode::Unknown => EntryMode::Unknown,
        }
    }
}

/// Metadata carries all metadata associated with a path.
#[napi]
pub struct Metadata(opendal::Metadata);

#[napi]
impl Metadata {
    /// Returns true if the <op.stat> object describes a file system directory.
    #[napi]
    pub fn is_directory(&self) -> bool {
        self.0.is_dir()
    }

    /// Returns true if the <op.stat> object describes a regular file.
    #[napi]
    pub fn is_file(&self) -> bool {
        self.0.is_file()
    }

    /// This function returns `true` if the file represented by this metadata has been marked for
    /// deletion or has been permanently deleted.
    #[napi]
    pub fn is_deleted(&self) -> bool {
        self.0.is_deleted()
    }

    /// Cache-Control of this object.
    #[napi(getter)]
    pub fn cache_control(&self) -> Option<String> {
        self.0.cache_control().map(|s| s.to_string())
    }

    /// Content-Disposition of this object
    #[napi(getter)]
    pub fn content_disposition(&self) -> Option<String> {
        self.0.content_disposition().map(|s| s.to_string())
    }

    /// Content Length of this object
    #[napi(getter)]
    pub fn content_length(&self) -> Option<u64> {
        self.0.content_length().into()
    }

    /// Content Encoding of this object
    #[napi(getter)]
    pub fn content_encoding(&self) -> Option<String> {
        self.0.content_encoding().map(|s| s.to_string())
    }

    /// Content MD5 of this object.
    #[napi(getter)]
    pub fn content_md5(&self) -> Option<String> {
        self.0.content_md5().map(|s| s.to_string())
    }

    /// Content Type of this object.
    #[napi(getter)]
    pub fn content_type(&self) -> Option<String> {
        self.0.content_type().map(|s| s.to_string())
    }

    /// User Metadata of this object.
    #[napi(getter)]
    pub fn user_metadata(&self) -> Option<HashMap<String, String>> {
        self.0.user_metadata().map(|metadata| {
            metadata
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        })
    }

    /// ETag of this object.
    #[napi(getter)]
    pub fn etag(&self) -> Option<String> {
        self.0.etag().map(|s| s.to_string())
    }

    /// Last Modified of this object.
    ///
    /// We will output this time in RFC3339 format like `1996-12-19T16:39:57+08:00`.
    #[napi(getter)]
    pub fn last_modified(&self) -> Option<String> {
        self.0.last_modified().map(|ta| ta.to_string())
    }

    /// mode represent this entry's mode.
    #[napi(getter)]
    pub fn mode(&self) -> Option<EntryMode> {
        Some(self.0.mode().into())
    }

    /// Retrieves the `version` of the file, if available.
    #[napi(getter)]
    pub fn version(&self) -> Option<String> {
        self.0.version().map(|v| v.to_string())
    }
}

/// BlockingReader is designed to read data from a given path in a blocking
/// manner.
#[napi]
pub struct BlockingReader {
    inner: opendal::blocking::StdReader,
}

#[napi]
impl BlockingReader {
    #[napi]
    pub fn read(&mut self, mut buf: Buffer) -> Result<usize> {
        let buf = buf.as_mut();
        let n = self.inner.read(buf).map_err(format_napi_error)?;
        Ok(n)
    }
}

/// Reader is designed to read data from a given path in an asynchronous
/// manner.
#[napi]
pub struct Reader {
    inner: opendal::FuturesAsyncReader,
}

#[napi]
impl Reader {
    /// # Safety
    ///
    /// > &mut self in async napi methods should be marked as unsafe
    ///
    /// Read bytes from this reader into given buffer.
    #[napi]
    pub async unsafe fn read(&mut self, mut buf: Buffer) -> Result<usize> {
        let buf = buf.as_mut();
        let n = self.inner.read(buf).await.map_err(format_napi_error)?;
        Ok(n)
    }
}

/// BlockingWriter is designed to write data into a given path in a blocking
/// manner.
#[napi]
pub struct BlockingWriter(opendal::blocking::Writer);

#[napi]
impl BlockingWriter {
    /// # Safety
    ///
    /// > &mut self in async napi methods should be marked as unsafe
    ///
    /// Write bytes into this writer.
    ///
    /// ### Example
    /// ```javascript
    /// const writer = await op.writer("path/to/file");
    /// await writer.write(Buffer.from("hello world"));
    /// await writer.close();
    /// ```
    #[napi]
    pub unsafe fn write(&mut self, content: Either<Buffer, String>) -> Result<()> {
        let c = match content {
            Either::A(buf) => buf.as_ref().to_owned(),
            Either::B(s) => s.into_bytes(),
        };
        self.0.write(c).map_err(format_napi_error)
    }

    /// # Safety
    ///
    /// > &mut self in async napi methods should be marked as unsafe
    ///
    /// Close this writer.
    ///
    /// ### Example
    ///
    /// ```javascript
    /// const writer = op.writerSync("path/to/file");
    /// writer.write(Buffer.from("hello world"));
    /// writer.close();
    /// ```
    #[napi]
    pub unsafe fn close(&mut self) -> Result<()> {
        self.0.close().map(|_| ()).map_err(format_napi_error)
    }
}

/// Writer is designed to write data into a given path in an asynchronous
/// manner.
#[napi]
pub struct Writer(opendal::Writer);

#[napi]
impl Writer {
    /// # Safety
    ///
    /// > &mut self in async napi methods should be marked as unsafe
    ///
    /// Write bytes into this writer.
    ///
    /// ### Example
    /// ```javascript
    /// const writer = await op.writer("path/to/file");
    /// await writer.write(Buffer.from("hello world"));
    /// await writer.close();
    /// ```
    #[napi]
    pub async unsafe fn write(&mut self, content: Either<Buffer, String>) -> Result<()> {
        let c = match content {
            Either::A(buf) => buf.as_ref().to_owned(),
            Either::B(s) => s.into_bytes(),
        };
        self.0.write(c).await.map_err(format_napi_error)
    }

    /// # Safety
    ///
    /// > &mut self in async napi methods should be marked as unsafe
    ///
    /// Close this writer.
    ///
    /// ### Example
    /// ```javascript
    /// const writer = await op.writer("path/to/file");
    /// await writer.write(Buffer.from("hello world"));
    /// await writer.close();
    /// ```
    #[napi]
    pub async unsafe fn close(&mut self) -> Result<()> {
        self.0.close().await.map(|_| ()).map_err(format_napi_error)
    }
}

/// Lister is designed to list entries at a given path in an asynchronous
/// manner.
#[napi]
pub struct Lister(opendal::Lister);

#[napi]
impl Lister {
    /// # Safety
    ///
    /// > &mut self in async napi methods should be marked as unsafe
    ///
    /// napi will make sure the function is safe, and we didn't do unsafe
    /// things internally.
    #[napi]
    pub async unsafe fn next(&mut self) -> Result<Option<Entry>> {
        Ok(self
            .0
            .try_next()
            .await
            .map_err(format_napi_error)?
            .map(Entry))
    }
}

/// BlockingLister is designed to list entries at a given path in a blocking
/// manner.
#[napi]
pub struct BlockingLister(opendal::blocking::Lister);

/// Method `next` can be confused for the standard trait method `std::iter::Iterator::next`.
/// But in JavaScript, it is also customary to use the next method directly to get the next element.
/// Therefore, disable this clippy.
/// It can be removed after a complete implementation of `Generator`.
/// FYI: <https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Generator>
#[napi]
#[allow(clippy::should_implement_trait)]
impl BlockingLister {
    #[napi]
    pub fn next(&mut self) -> Result<Option<Entry>> {
        match self.0.next() {
            Some(Ok(entry)) => Ok(Some(Entry(entry))),
            Some(Err(e)) => Err(format_napi_error(e)),
            None => Ok(None),
        }
    }
}

/// PresignedRequest is a presigned request return by `presign`.
#[napi(object)]
pub struct PresignedRequest {
    /// HTTP method of this request.
    pub method: String,
    /// URL of this request.
    pub url: String,
    /// HTTP headers of this request.
    pub headers: HashMap<String, String>,
}

impl PresignedRequest {
    pub fn new(req: opendal::raw::PresignedRequest) -> Self {
        let method = req.method().to_string();
        let url = req.uri().to_string();
        let headers = req
            .header()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str()
                        .expect("header value contains non visible ascii characters")
                        .to_string(),
                )
            })
            .collect();
        Self {
            method,
            url,
            headers,
        }
    }
}

#[napi]
impl Operator {
    /// Add a layer to this operator.
    #[napi(async_runtime)]
    pub fn layer(&self, layer: &External<Layer>) -> Result<Self> {
        let async_op = layer.inner.layer(self.async_op.clone());

        let blocking_op =
            opendal::blocking::Operator::new(async_op.clone()).map_err(format_napi_error)?;

        Ok(Self {
            async_op,
            blocking_op,
        })
    }
}

/// Format opendal error to napi error.
///
/// FIXME: handle error correctly.
fn format_napi_error(err: impl Display) -> Error {
    Error::from_reason(format!("{err}"))
}
