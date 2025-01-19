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
use std::str::FromStr;
use std::time::Duration;

use futures::AsyncReadExt;
use futures::TryStreamExt;
use napi::bindgen_prelude::*;

mod capability;

#[napi]
pub struct Operator(opendal::Operator);

#[napi]
impl Operator {
    /// @see For the full list of scheme, see https://docs.rs/opendal/latest/opendal/services/index.html
    /// And the options,
    /// please refer to the documentation of the corresponding service for the corresponding parameters.
    /// Note that the current options key is snake_case.
    #[napi(constructor)]
    pub fn new(scheme: String, options: Option<HashMap<String, String>>) -> Result<Self> {
        let scheme = opendal::Scheme::from_str(&scheme)
            .map_err(|err| {
                opendal::Error::new(opendal::ErrorKind::Unexpected, "not supported scheme")
                    .set_source(err)
            })
            .map_err(format_napi_error)?;
        let options = options.unwrap_or_default();

        let mut op = opendal::Operator::via_iter(scheme, options).map_err(format_napi_error)?;

        if !op.info().full_capability().blocking {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let _guard = handle.enter();
                op = op.layer(
                    opendal::layers::BlockingLayer::create()
                        .expect("blocking layer must be created"),
                );
            }
        }

        Ok(Operator(op))
    }

    /// Get current operator(service)'s full capability.
    #[napi]
    pub fn capability(&self) -> Result<capability::Capability> {
        Ok(capability::Capability::new(self.0.info().full_capability()))
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
    pub async fn stat(&self, path: String) -> Result<Metadata> {
        let meta = self.0.stat(&path).await.map_err(format_napi_error)?;

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
    pub fn stat_sync(&self, path: String) -> Result<Metadata> {
        let meta = self.0.blocking().stat(&path).map_err(format_napi_error)?;

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
        self.0.check().await.map_err(format_napi_error)
    }

    /// Check if this path exists or not.
    ///
    /// ### Example
    /// ```javascript
    /// await op.isExist("test");
    /// ```
    #[napi]
    pub async fn is_exist(&self, path: String) -> Result<bool> {
        self.0.is_exist(&path).await.map_err(format_napi_error)
    }

    /// Check if this path exists or not synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.isExistSync("test");
    /// ```
    #[napi]
    pub fn is_exist_sync(&self, path: String) -> Result<bool> {
        self.0.blocking().is_exist(&path).map_err(format_napi_error)
    }

    /// Create dir with a given path.
    ///
    /// ### Example
    /// ```javascript
    /// await op.createDir("path/to/dir/");
    /// ```
    #[napi]
    pub async fn create_dir(&self, path: String) -> Result<()> {
        self.0.create_dir(&path).await.map_err(format_napi_error)
    }

    /// Create dir with a given path synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.createDirSync("path/to/dir/");
    /// ```
    #[napi]
    pub fn create_dir_sync(&self, path: String) -> Result<()> {
        self.0
            .blocking()
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
    pub async fn read(&self, path: String) -> Result<Buffer> {
        let res = self
            .0
            .read(&path)
            .await
            .map_err(format_napi_error)?
            .to_vec();
        Ok(res.into())
    }

    /// Create a reader to read the given path.
    ///
    /// It could be used to read large file in a streaming way.
    #[napi]
    pub async fn reader(&self, path: String) -> Result<Reader> {
        let r = self.0.reader(&path).await.map_err(format_napi_error)?;
        Ok(Reader {
            inner: r
                .into_futures_async_read(..)
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
    pub fn read_sync(&self, path: String) -> Result<Buffer> {
        let res = self
            .0
            .blocking()
            .read(&path)
            .map_err(format_napi_error)?
            .to_vec();
        Ok(res.into())
    }

    /// Create a reader to read the given path synchronously.
    ///
    /// It could be used to read large file in a streaming way.
    #[napi]
    pub fn reader_sync(&self, path: String) -> Result<BlockingReader> {
        let r = self.0.blocking().reader(&path).map_err(format_napi_error)?;
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
        options: Option<WriteOptions>,
    ) -> Result<()> {
        let c = match content {
            Either::A(buf) => buf.as_ref().to_owned(),
            Either::B(s) => s.into_bytes(),
        };
        let mut writer = self.0.write_with(&path, c);
        if let Some(options) = options {
            if let Some(append) = options.append {
                writer = writer.append(append);
            }
            if let Some(chunk) = options.chunk {
                writer = writer.chunk(chunk.get_u64().1 as usize);
            }
            if let Some(ref content_type) = options.content_type {
                writer = writer.content_type(content_type);
            }
            if let Some(ref content_disposition) = options.content_disposition {
                writer = writer.content_disposition(content_disposition);
            }
            if let Some(ref cache_control) = options.cache_control {
                writer = writer.cache_control(cache_control);
            }
        }
        writer.await.map(|_| ()).map_err(format_napi_error)
    }

    //noinspection DuplicatedCode
    /// Write multiple bytes into a path.
    ///
    /// It could be used to write large file in a streaming way.
    #[napi]
    pub async fn writer(&self, path: String, options: Option<WriterOptions>) -> Result<Writer> {
        let mut writer = self.0.writer_with(&path);
        if let Some(options) = options {
            if let Some(append) = options.append {
                writer = writer.append(append);
            }
            if let Some(chunk) = options.chunk {
                writer = writer.chunk(chunk.get_u64().1 as usize);
            }
            if let Some(ref content_type) = options.content_type {
                writer = writer.content_type(content_type);
            }
            if let Some(ref content_disposition) = options.content_disposition {
                writer = writer.content_disposition(content_disposition);
            }
            if let Some(ref cache_control) = options.cache_control {
                writer = writer.cache_control(cache_control);
            }
        }
        let w = writer.await.map_err(format_napi_error)?;
        Ok(Writer(w))
    }

    /// Write multiple bytes into a path synchronously.
    ///
    /// It could be used to write large file in a streaming way.
    #[napi]
    pub fn writer_sync(
        &self,
        path: String,
        options: Option<WriterOptions>,
    ) -> Result<BlockingWriter> {
        let mut writer = self.0.blocking().writer_with(&path);
        if let Some(options) = options {
            if let Some(append) = options.append {
                writer = writer.append(append);
            }
            if let Some(chunk) = options.chunk {
                writer = writer.chunk(chunk.get_u64().1 as usize);
            }
            if let Some(ref content_type) = options.content_type {
                writer = writer.content_type(content_type);
            }
            if let Some(ref content_disposition) = options.content_disposition {
                writer = writer.content_disposition(content_disposition);
            }
            if let Some(ref cache_control) = options.cache_control {
                writer = writer.cache_control(cache_control);
            }
        }
        let w = writer.call().map_err(format_napi_error)?;
        Ok(BlockingWriter(w))
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
        options: Option<WriteOptions>,
    ) -> Result<()> {
        let c = match content {
            Either::A(buf) => buf.as_ref().to_owned(),
            Either::B(s) => s.into_bytes(),
        };
        let mut writer = self.0.blocking().write_with(&path, c);
        if let Some(options) = options {
            if let Some(append) = options.append {
                writer = writer.append(append);
            }
            if let Some(chunk) = options.chunk {
                writer = writer.chunk(chunk.get_u64().1 as usize);
            }
            if let Some(ref content_type) = options.content_type {
                writer = writer.content_type(content_type);
            }
            if let Some(ref content_disposition) = options.content_disposition {
                writer = writer.content_disposition(content_disposition);
            }
            if let Some(ref cache_control) = options.cache_control {
                writer = writer.cache_control(cache_control);
            }
        }
        writer.call().map(|_| ()).map_err(format_napi_error)
    }

    /// Copy file according to given `from` and `to` path.
    ///
    /// ### Example
    /// ```javascript
    /// await op.copy("path/to/file", "path/to/dest");
    /// ```
    #[napi]
    pub async fn copy(&self, from: String, to: String) -> Result<()> {
        self.0.copy(&from, &to).await.map_err(format_napi_error)
    }

    /// Copy file according to given `from` and `to` path synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.copySync("path/to/file", "path/to/dest");
    /// ```
    #[napi]
    pub fn copy_sync(&self, from: String, to: String) -> Result<()> {
        self.0
            .blocking()
            .copy(&from, &to)
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
        self.0.rename(&from, &to).await.map_err(format_napi_error)
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
        self.0
            .blocking()
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
    pub async fn delete(&self, path: String) -> Result<()> {
        self.0.delete(&path).await.map_err(format_napi_error)
    }

    /// Delete the given path synchronously.
    ///
    /// ### Example
    /// ```javascript
    /// op.deleteSync("test");
    /// ```
    #[napi]
    pub fn delete_sync(&self, path: String) -> Result<()> {
        self.0.blocking().delete(&path).map_err(format_napi_error)
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
        self.0.remove(paths).await.map_err(format_napi_error)
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
        self.0.remove_all(&path).await.map_err(format_napi_error)
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
    pub async fn list(&self, path: String, options: Option<ListOptions>) -> Result<Vec<Entry>> {
        let mut l = self.0.list_with(&path);
        if let Some(options) = options {
            if let Some(limit) = options.limit {
                l = l.limit(limit as usize);
            }
            if let Some(recursive) = options.recursive {
                l = l.recursive(recursive);
            }
        }

        Ok(l.await
            .map_err(format_napi_error)?
            .iter()
            .map(|e| Entry(e.to_owned()))
            .collect())
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
    ///   let meta = op.statSync(entry.path);
    ///   if (meta.isFile) {
    ///     // do something
    ///   }
    /// }
    /// ```
    #[napi]
    pub fn list_sync(&self, path: String, options: Option<ListOptions>) -> Result<Vec<Entry>> {
        let mut l = self.0.blocking().list_with(&path);
        if let Some(options) = options {
            if let Some(limit) = options.limit {
                l = l.limit(limit as usize);
            }
            if let Some(recursive) = options.recursive {
                l = l.recursive(recursive);
            }
        }

        Ok(l.call()
            .map_err(format_napi_error)?
            .iter()
            .map(|e| Entry(e.to_owned()))
            .collect())
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
            .0
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
            .0
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
            .0
            .presign_stat(&path, Duration::from_secs(expires as u64))
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
        self.0.last_modified().map(|ta| ta.to_rfc3339())
    }
}

#[napi(object)]
pub struct ListOptions {
    pub limit: Option<u32>,
    pub recursive: Option<bool>,
}

/// BlockingReader is designed to read data from a given path in a blocking
/// manner.
#[napi]
pub struct BlockingReader {
    inner: opendal::StdReader,
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
pub struct BlockingWriter(opendal::BlockingWriter);

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

#[napi(object)]
#[derive(Default)]
pub struct WriteOptions {
    /// Append bytes into a path.
    ///
    /// ### Notes
    ///
    /// - It always appends content to the end of the file.
    /// - It will create file if the path does not exist.
    pub append: Option<bool>,

    /// Set the chunk of op.
    ///
    /// If chunk is set, the data will be chunked by the underlying writer.
    ///
    /// ## NOTE
    ///
    /// A service could have their own minimum chunk size while perform write
    /// operations like multipart uploads. So the chunk size may be larger than
    /// the given buffer size.
    pub chunk: Option<BigInt>,

    /// Set the [Content-Type](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type) of op.
    pub content_type: Option<String>,

    /// Set the [Content-Disposition](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition) of op.
    pub content_disposition: Option<String>,

    /// Set the [Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control) of op.
    pub cache_control: Option<String>,
}

#[napi(object)]
#[derive(Default)]
pub struct WriterOptions {
    /// Append bytes into a path.
    ///
    /// ### Notes
    ///
    /// - It always appends content to the end of the file.
    /// - It will create file if the path does not exist.
    pub append: Option<bool>,

    /// Set the chunk of op.
    ///
    /// If chunk is set, the data will be chunked by the underlying writer.
    ///
    /// ## NOTE
    ///
    /// A service could have their own minimum chunk size while perform write
    /// operations like multipart uploads. So the chunk size may be larger than
    /// the given buffer size.
    pub chunk: Option<BigInt>,

    /// Set the [Content-Type](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Type) of op.
    pub content_type: Option<String>,

    /// Set the [Content-Disposition](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Content-Disposition) of op.
    pub content_disposition: Option<String>,

    /// Set the [Cache-Control](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Cache-Control) of op.
    pub cache_control: Option<String>,
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
pub struct BlockingLister(opendal::BlockingLister);

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

pub trait NodeLayer: Send + Sync {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator;
}

/// A public layer wrapper
#[napi]
pub struct Layer {
    inner: Box<dyn NodeLayer>,
}

#[napi]
impl Operator {
    /// Add a layer to this operator.
    #[napi]
    pub fn layer(&self, layer: External<Layer>) -> Result<Self> {
        Ok(Self(layer.inner.layer(self.0.clone())))
    }
}

impl NodeLayer for opendal::layers::RetryLayer {
    fn layer(&self, op: opendal::Operator) -> opendal::Operator {
        op.layer(self.clone())
    }
}

/// Retry layer
///
/// Add retry for temporary failed operations.
///
/// # Notes
///
/// This layer will retry failed operations when [`Error::is_temporary`]
/// returns true.
/// If the operation still failed, this layer will set error to
/// `Persistent` which means error has been retried.
///
/// `write` and `blocking_write` don't support retry so far,
/// visit [this issue](https://github.com/apache/opendal/issues/1223) for more details.
///
/// # Examples
///
/// ```javascript
/// const op = new Operator("file", { root: "/tmp" })
///
/// const retry = new RetryLayer();
/// retry.max_times = 3;
/// retry.jitter = true;
///
/// op.layer(retry.build());
/// ```
#[derive(Default)]
#[napi]
pub struct RetryLayer {
    jitter: bool,
    max_times: Option<u32>,
    factor: Option<f64>,
    max_delay: Option<f64>,
    min_delay: Option<f64>,
}

#[napi]
impl RetryLayer {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set jitter of current backoff.
    ///
    /// If jitter is enabled, ExponentialBackoff will add a random jitter in `[0, min_delay)`
    /// to current delay.
    #[napi(setter)]
    pub fn jitter(&mut self, v: bool) {
        self.jitter = v;
    }

    /// Set max_times of current backoff.
    ///
    /// Backoff will return `None` if max times are reached.
    #[napi(setter)]
    pub fn max_times(&mut self, v: u32) {
        self.max_times = Some(v);
    }

    /// Set factor of current backoff.
    ///
    /// # Panics
    ///
    /// This function will panic if the input factor is smaller than `1.0`.
    #[napi(setter)]
    pub fn factor(&mut self, v: f64) {
        self.factor = Some(v);
    }

    /// Set max_delay of current backoff.
    ///
    /// Delay will not increase if the current delay is larger than max_delay.
    ///
    /// # Notes
    ///
    /// - The unit of max_delay is millisecond.
    #[napi(setter)]
    pub fn max_delay(&mut self, v: f64) {
        self.max_delay = Some(v);
    }

    /// Set min_delay of current backoff.
    ///
    /// # Notes
    ///
    /// - The unit of min_delay is millisecond.
    #[napi(setter)]
    pub fn min_delay(&mut self, v: f64) {
        self.min_delay = Some(v);
    }

    #[napi]
    pub fn build(&self) -> External<Layer> {
        let mut l = opendal::layers::RetryLayer::default();
        if self.jitter {
            l = l.with_jitter();
        }
        if let Some(max_times) = self.max_times {
            l = l.with_max_times(max_times as usize);
        }
        if let Some(factor) = self.factor {
            l = l.with_factor(factor as f32);
        }
        if let Some(max_delay) = self.max_delay {
            l = l.with_max_delay(Duration::from_millis(max_delay as u64));
        }
        if let Some(min_delay) = self.min_delay {
            l = l.with_min_delay(Duration::from_millis(min_delay as u64));
        }

        External::new(Layer { inner: Box::new(l) })
    }
}

/// Format opendal error to napi error.
///
/// FIXME: handle error correctly.
fn format_napi_error(err: impl Display) -> Error {
    Error::from_reason(format!("{}", err))
}
