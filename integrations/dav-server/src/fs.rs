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

use dav_server::davpath::DavPath;
use dav_server::fs::DavMetaData;
use dav_server::fs::FsError;
use dav_server::fs::{DavDirEntry, FsFuture};
use dav_server::fs::{DavFile, FsStream};
use dav_server::fs::{DavFileSystem, ReadDirMeta};
use futures::FutureExt;
use futures::StreamExt;
use opendal_core::raw::normalize_path;
use opendal_core::{ErrorKind, Operator};
use std::path::Path;

use super::dir::OpendalStream;
use super::file::OpendalFile;
use super::metadata::OpendalMetaData;
use super::utils::convert_error;

/// OpendalFs is a `DavFileSystem` implementation for opendal.
///
/// ```
/// use anyhow::Result;
/// use dav_server::davpath::DavPath;
/// use dav_server::fs::DavFileSystem;
/// use dav_server_opendalfs::OpendalFs;
/// use opendal_core::services::Memory;
/// use opendal_core::Operator;
///
/// #[tokio::test]
/// async fn test() -> Result<()> {
///     let op = Operator::new(Memory::default())?;
///
///     let webdavfs = OpendalFs::new(op);
///
///     let metadata = webdavfs
///         .metadata(&DavPath::new("/").unwrap())
///         .await
///         .unwrap();
///     println!("{}", metadata.is_dir());
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct OpendalFs {
    pub op: Operator,
}

impl OpendalFs {
    /// Create a new `OpendalFs` instance.
    pub fn new(op: Operator) -> Box<OpendalFs> {
        Box::new(OpendalFs { op })
    }

    fn fs_path(&self, path: &DavPath) -> Result<String, FsError> {
        String::from_utf8(path.as_bytes().to_vec())
            .map(|path| normalize_path(path.as_str()))
            .map_err(|_| FsError::GeneralFailure)
    }

    fn fs_dir_path(&self, path: &DavPath) -> Result<String, FsError> {
        Ok(Self::into_dir_path(self.fs_path(path)?))
    }

    fn into_dir_path(mut path: String) -> String {
        if !Self::is_dir_path(&path) {
            path.push('/');
        }
        path
    }

    fn is_dir_path(path: &str) -> bool {
        path.is_empty() || path.ends_with('/')
    }
}

impl DavFileSystem for OpendalFs {
    fn open<'a>(
        &'a self,
        path: &'a DavPath,
        options: dav_server::fs::OpenOptions,
    ) -> FsFuture<'a, Box<dyn DavFile>> {
        async move {
            let path = self.fs_path(path)?;
            let file = OpendalFile::open(self.op.clone(), path, options).await?;
            Ok(Box::new(file) as Box<dyn DavFile>)
        }
        .boxed()
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
    ) -> FsFuture<'a, FsStream<Box<dyn DavDirEntry>>> {
        async move {
            let path = self.fs_dir_path(path)?;
            self.op
                .lister(path.as_str())
                .await
                .map(|lister| OpendalStream::new(self.op.clone(), lister, path.as_str()).boxed())
                .map_err(convert_error)
        }
        .boxed()
    }

    fn metadata<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, Box<dyn DavMetaData>> {
        async move {
            let path = self.fs_path(path)?;
            let metadata = match self.op.stat(path.as_str()).await {
                Ok(metadata) => metadata,
                Err(e)
                    if !Self::is_dir_path(&path)
                        && matches!(e.kind(), ErrorKind::NotFound | ErrorKind::IsADirectory) =>
                {
                    let path = Self::into_dir_path(path);
                    self.op.stat(path.as_str()).await.map_err(convert_error)?
                }
                Err(e) => return Err(convert_error(e)),
            };

            Ok(Box::new(OpendalMetaData::new(metadata)) as Box<dyn DavMetaData>)
        }
        .boxed()
    }

    fn create_dir<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        async move {
            let path = self.fs_dir_path(path)?;

            // check if the parent path is exist.
            // During MKCOL processing, a server MUST make the Request-URI a member of its parent collection, unless the Request-URI is "/".  If no such ancestor exists, the method MUST fail.
            // refer to https://datatracker.ietf.org/doc/html/rfc2518#section-8.3.1
            let parent = Path::new(&path).parent().unwrap();
            match self
                .op
                .exists(format!("{}/", parent.display()).as_str())
                .await
            {
                Ok(exist) => {
                    if !exist && parent != Path::new("/") {
                        return Err(FsError::NotFound);
                    }
                }
                Err(e) => {
                    return Err(convert_error(e));
                }
            }

            let path = path.as_str();
            // check if the given path is exist (MKCOL on existing collection should fail (RFC2518:8.3.1))
            let exist = self.op.exists(path).await;
            match exist {
                Ok(exist) => match exist {
                    true => Err(FsError::Exists),
                    false => {
                        let res = self.op.create_dir(path).await;
                        match res {
                            Ok(_) => Ok(()),
                            Err(e) => Err(convert_error(e)),
                        }
                    }
                },
                Err(e) => Err(convert_error(e)),
            }
        }
        .boxed()
    }

    fn remove_dir<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        async move {
            let path = self.fs_dir_path(path)?;
            self.op.delete(&path).await.map_err(convert_error)
        }
        .boxed()
    }

    fn remove_file<'a>(&'a self, path: &'a DavPath) -> FsFuture<'a, ()> {
        async move {
            let path = self.fs_path(path)?;
            self.op.delete(&path).await.map_err(convert_error)
        }
        .boxed()
    }

    fn rename<'a>(&'a self, from: &'a DavPath, to: &'a DavPath) -> FsFuture<'a, ()> {
        async move {
            if from.is_collection() {
                let from_path = self.fs_dir_path(from)?;
                let to_path = self.fs_dir_path(to)?;

                if self
                    .op
                    .exists(to_path.as_str())
                    .await
                    .map_err(convert_error)?
                {
                    return Err(FsError::Exists);
                }

                let mut lister = self
                    .op
                    .lister_with(from_path.as_str())
                    .limit(2)
                    .await
                    .map_err(convert_error)?;
                let mut found = false;
                while let Some(entry) = lister.next().await {
                    let entry = entry.map_err(convert_error)?;
                    if entry.path() == from_path {
                        found = true;
                    } else {
                        // Only empty directory moves are supported.
                        return Err(FsError::NotImplemented);
                    }
                }
                if !found {
                    return Err(FsError::NotFound);
                }

                self.op
                    .create_dir(to_path.as_str())
                    .await
                    .map_err(convert_error)?;
                return self
                    .op
                    .delete(from_path.as_str())
                    .await
                    .map_err(convert_error);
            }

            let from_path = self.fs_path(from)?;
            let to_path = self.fs_path(to)?;
            self.op
                .rename(from_path.as_str(), to_path.as_str())
                .await
                .map_err(convert_error)
        }
        .boxed()
    }

    fn copy<'a>(&'a self, from: &'a DavPath, to: &'a DavPath) -> FsFuture<'a, ()> {
        async move {
            let from_path = self.fs_path(from)?;
            let to_path = self.fs_path(to)?;
            self.op
                .copy(from_path.as_str(), to_path.as_str())
                .await
                .map(|_| ())
                .map_err(convert_error)
        }
        .boxed()
    }
}
