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

//! `unftp-sbe-opendal` is an [unftp](https://crates.io/crates/unftp) `StorageBackend` implementation using opendal.
//!
//! This crate can help you to access ANY storage services with the same ftp API.
//!
//! # Example
//!
//! This example demonstrates how to use `unftp-sbe-opendal` with the `S3` service.
//!
//! ```no_run
//! use anyhow::Result;
//! use opendal::Operator;
//! use opendal::services;
//! use unftp_sbe_opendal::OpendalStorage;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Create any service desired
//!     let op = opendal::Operator::from_iter::<services::S3>(
//!         [
//!             ("bucket".to_string(), "my_bucket".to_string()),
//!             ("access_key".to_string(), "my_access_key".to_string()),
//!             ("secret_key".to_string(), "my_secret_key".to_string()),
//!             ("endpoint".to_string(), "my_endpoint".to_string()),
//!             ("region".to_string(), "my_region".to_string()),
//!         ],
//!     )?;
//!
//!     // Wrap the operator with `OpendalStorage`
//!     let backend = OpendalStorage::new(op);
//!
//!     // Build the actual unftp server
//!     let server = libunftp::ServerBuilder::new(Box::new(move || backend.clone())).build()?;
//!
//!     // Start the server
//!     server.listen("0.0.0.0:0").await?;
//!
//!     Ok(())
//! }
//! ```

use std::fmt::Debug;
use std::path::{Path, PathBuf};

use opendal::Operator;
use unftp_core::auth::UserDetail;
use unftp_core::storage::{self, Error, StorageBackend};

use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio_util::compat::{FuturesAsyncReadCompatExt, FuturesAsyncWriteCompatExt};

#[derive(Debug, Clone)]
pub struct OpendalStorage {
    op: Operator,
}

impl OpendalStorage {
    pub fn new(op: Operator) -> Self {
        Self { op }
    }
}

/// A wrapper around [`opendal::Metadata`] to implement [`storage::Metadata`].
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OpendalMetadata(opendal::Metadata);

impl storage::Metadata for OpendalMetadata {
    fn len(&self) -> u64 {
        self.0.content_length()
    }

    fn is_dir(&self) -> bool {
        self.0.is_dir()
    }

    fn is_file(&self) -> bool {
        self.0.is_file()
    }

    fn is_symlink(&self) -> bool {
        false
    }

    fn modified(&self) -> storage::Result<std::time::SystemTime> {
        match self.0.last_modified() {
            Some(ts) => Ok(ts.into()),
            None => Err(Error::new(
                storage::ErrorKind::LocalError,
                "no last modified time",
            )),
        }
    }

    fn gid(&self) -> u32 {
        0
    }

    fn uid(&self) -> u32 {
        0
    }
}

fn convert_err(err: opendal::Error) -> Error {
    let kind = match err.kind() {
        opendal::ErrorKind::NotFound => storage::ErrorKind::PermanentFileNotAvailable,
        opendal::ErrorKind::AlreadyExists => storage::ErrorKind::PermanentFileNotAvailable,
        opendal::ErrorKind::PermissionDenied => storage::ErrorKind::PermissionDenied,
        _ => storage::ErrorKind::LocalError,
    };
    Error::new(kind, err)
}

fn convert_path(path: &Path) -> storage::Result<&str> {
    path.to_str().ok_or_else(|| {
        Error::new(
            storage::ErrorKind::LocalError,
            "Path is not a valid UTF-8 string",
        )
    })
}

async fn copy_read_write_loop<R, W>(input: &mut R, output: &mut W) -> std::io::Result<u64>
where
    R: tokio::io::AsyncRead + Unpin + ?Sized,
    W: tokio::io::AsyncWrite + Unpin + ?Sized,
{
    let mut copied = 0u64;
    let mut buf = [0u8; 8 * 1024];

    loop {
        let n = input.read(&mut buf).await?;
        if n == 0 {
            return Ok(copied);
        }

        output.write_all(&buf[..n]).await?;
        copied += n as u64;
    }
}

#[async_trait::async_trait]
impl<User: UserDetail> StorageBackend<User> for OpendalStorage {
    type Metadata = OpendalMetadata;

    async fn metadata<P: AsRef<Path> + Send + Debug>(
        &self,
        _: &User,
        path: P,
    ) -> storage::Result<Self::Metadata> {
        let metadata = self
            .op
            .stat(convert_path(path.as_ref())?)
            .await
            .map_err(convert_err)?;
        Ok(OpendalMetadata(metadata))
    }

    async fn list<P: AsRef<Path> + Send + Debug>(
        &self,
        _: &User,
        path: P,
    ) -> storage::Result<Vec<storage::Fileinfo<PathBuf, Self::Metadata>>>
    where
        Self::Metadata: storage::Metadata,
    {
        let ret = self
            .op
            .list(convert_path(path.as_ref())?)
            .await
            .map_err(convert_err)?
            .into_iter()
            .map(|x| {
                let (path, metadata) = x.into_parts();
                storage::Fileinfo {
                    path: path.into(),
                    metadata: OpendalMetadata(metadata),
                }
            })
            .collect();
        Ok(ret)
    }

    async fn get<P: AsRef<Path> + Send + Debug>(
        &self,
        _: &User,
        path: P,
        start_pos: u64,
    ) -> storage::Result<Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>> {
        let reader = self
            .op
            .reader(convert_path(path.as_ref())?)
            .await
            .map_err(convert_err)?
            .into_futures_async_read(start_pos..)
            .await
            .map_err(convert_err)?
            .compat();
        Ok(Box::new(reader))
    }

    async fn put<
        P: AsRef<Path> + Send + Debug,
        R: tokio::io::AsyncRead + Send + Sync + Unpin + 'static,
    >(
        &self,
        _: &User,
        mut input: R,
        path: P,
        _: u64,
    ) -> storage::Result<u64> {
        let mut w = self
            .op
            .writer(convert_path(path.as_ref())?)
            .await
            .map_err(convert_err)?
            .into_futures_async_write()
            .compat_write();
        // Avoid `tokio::io::copy`'s pending-read flush path and keep buffering policy explicit.
        let copy_result = copy_read_write_loop(&mut input, &mut w).await;
        let shutdown_result = w.shutdown().await;
        match (copy_result, shutdown_result) {
            (Ok(len), Ok(())) => Ok(len),
            (Err(copy_err), Ok(())) => Err(Error::new(
                storage::ErrorKind::LocalError,
                format!("Failed to copy data: {}", copy_err),
            )),
            (Ok(_), Err(shutdown_err)) => Err(Error::new(
                storage::ErrorKind::LocalError,
                format!("Failed to shutdown writer: {}", shutdown_err),
            )),
            (Err(copy_err), Err(shutdown_err)) => Err(Error::new(
                storage::ErrorKind::LocalError,
                format!(
                    "Failed to copy data: {} AND failed to shutdown writer: {}",
                    copy_err, shutdown_err
                ),
            )),
        }
    }

    async fn del<P: AsRef<Path> + Send + Debug>(&self, _: &User, path: P) -> storage::Result<()> {
        self.op
            .delete(convert_path(path.as_ref())?)
            .await
            .map_err(convert_err)
    }

    async fn mkd<P: AsRef<Path> + Send + Debug>(&self, _: &User, path: P) -> storage::Result<()> {
        let mut path_str = convert_path(path.as_ref())?.to_string();
        if !path_str.ends_with('/') {
            path_str.push('/');
        }
        self.op.create_dir(&path_str).await.map_err(convert_err)
    }

    async fn rename<P: AsRef<Path> + Send + Debug>(
        &self,
        _: &User,
        from: P,
        to: P,
    ) -> storage::Result<()> {
        let (from, to) = (convert_path(from.as_ref())?, convert_path(to.as_ref())?);
        self.op.rename(from, to).await.map_err(convert_err)
    }

    async fn rmd<P: AsRef<Path> + Send + Debug>(&self, _: &User, path: P) -> storage::Result<()> {
        self.op
            .delete_with(convert_path(path.as_ref())?)
            .recursive(true)
            .await
            .map_err(convert_err)
    }

    async fn cwd<P: AsRef<Path> + Send + Debug>(&self, _: &User, path: P) -> storage::Result<()> {
        use opendal::ErrorKind::*;

        match self.op.stat(convert_path(path.as_ref())?).await {
            Ok(_) => Ok(()),
            Err(e) if matches!(e.kind(), NotFound | NotADirectory) => Err(Error::new(
                storage::ErrorKind::PermanentDirectoryNotAvailable,
                e,
            )),
            Err(e) => Err(convert_err(e)),
        }
    }
}
