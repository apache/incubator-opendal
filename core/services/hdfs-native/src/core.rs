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
use std::sync::Arc;

use hdfs_native::HdfsError;
use hdfs_native::WriteOptions;

use opendal_core::raw::*;
use opendal_core::*;

/// HdfsNativeCore contains code that directly interacts with HDFS Native client.
#[derive(Clone)]
pub struct HdfsNativeCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub root: String,
    pub client: Arc<hdfs_native::Client>,
}

impl Debug for HdfsNativeCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdfsNativeCore")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl HdfsNativeCore {
    pub async fn hdfs_create_dir(&self, path: &str) -> Result<()> {
        let p = build_rooted_abs_path(&self.root, path);

        self.client
            .mkdirs(&p, 0o777, true)
            .await
            .map_err(parse_hdfs_error)?;

        Ok(())
    }

    pub async fn hdfs_stat(&self, path: &str) -> Result<Metadata> {
        let p = build_rooted_abs_path(&self.root, path);

        let status: hdfs_native::client::FileStatus = self
            .client
            .get_file_info(&p)
            .await
            .map_err(parse_hdfs_error)?;

        let mode = if status.isdir {
            EntryMode::DIR
        } else {
            EntryMode::FILE
        };

        let mut metadata = if mode == EntryMode::FILE {
            MetadataBuilder::file(status.length as u64)
        } else {
            MetadataBuilder::dir()
        };
        metadata.last_modified(Timestamp::from_millisecond(
            status.modification_time as i64,
        )?);

        Ok(metadata.build())
    }

    pub async fn hdfs_open(&self, path: &str) -> Result<hdfs_native::file::FileReader> {
        let p = build_rooted_abs_path(&self.root, path);

        self.client.read(&p).await.map_err(parse_hdfs_error)
    }

    pub async fn hdfs_write(
        &self,
        path: &str,
        args: &OpWrite,
    ) -> Result<(hdfs_native::file::FileWriter, u64)> {
        let target_path = build_rooted_abs_path(&self.root, path);
        let mut initial_size = 0;

        let target_exists = match self.client.get_file_info(&target_path).await {
            Ok(status) => {
                initial_size = status.length as u64;
                true
            }
            Err(err) => match &err {
                HdfsError::FileNotFound(_) => false,
                _ => return Err(parse_hdfs_error(err)),
            },
        };

        let f = if target_exists {
            if args.append() {
                self.client
                    .append(&target_path)
                    .await
                    .map_err(parse_hdfs_error)?
            } else {
                initial_size = 0;
                self.client
                    .create(&target_path, WriteOptions::default().overwrite(true))
                    .await
                    .map_err(parse_hdfs_error)?
            }
        } else {
            initial_size = 0;
            self.client
                .create(&target_path, WriteOptions::default())
                .await
                .map_err(parse_hdfs_error)?
        };

        Ok((f, initial_size))
    }

    pub async fn hdfs_delete(&self, path: &str) -> Result<()> {
        let p = build_rooted_abs_path(&self.root, path);

        self.client
            .delete(&p, true)
            .await
            .map_err(parse_hdfs_error)?;

        Ok(())
    }

    pub async fn hdfs_list(&self, path: &str) -> Result<Option<(String, Option<String>)>> {
        let p: String = build_rooted_abs_path(&self.root, path);

        let isdir = match self.client.get_file_info(&p).await {
            Ok(status) => status.isdir,
            Err(err) => {
                return match &err {
                    HdfsError::FileNotFound(_) => Ok(None),
                    _ => Err(parse_hdfs_error(err)),
                };
            }
        };

        let current_path = if isdir {
            if !path.ends_with("/") {
                Some(path.to_string() + "/")
            } else {
                Some(path.to_string())
            }
        } else {
            None
        };

        Ok(Some((p, current_path)))
    }

    pub async fn hdfs_rename(&self, from: &str, to: &str, args: &OpRename) -> Result<()> {
        let from_path = build_rooted_abs_path(&self.root, from);
        let to_path = build_rooted_abs_path(&self.root, to);

        let target_exists = match self.client.get_file_info(&to_path).await {
            Ok(status) => {
                if status.isdir {
                    return Err(Error::new(ErrorKind::IsADirectory, "path should be a file")
                        .with_context("input", &to_path));
                }
                true
            }
            Err(err) => match &err {
                HdfsError::FileNotFound(_) => false,
                _ => return Err(parse_hdfs_error(err)),
            },
        };

        if args.if_not_exists() {
            if let Some((parent, _)) = to_path
                .rsplit_once('/')
                .filter(|(parent, _)| !target_exists && !parent.is_empty())
            {
                self.client
                    .mkdirs(parent, 0o777, true)
                    .await
                    .map_err(parse_hdfs_error)?;
            }
            return self
                .client
                .rename(&from_path, &to_path, false)
                .await
                .map_err(|err| {
                    if matches!(&err, HdfsError::AlreadyExists(_)) {
                        Error::new(
                            ErrorKind::ConditionNotMatch,
                            "target path already exists while if_not_exists is set",
                        )
                        .with_context("input", &to_path)
                        .set_source(err)
                    } else {
                        parse_hdfs_error(err)
                    }
                });
        }

        if target_exists {
            self.client
                .delete(&to_path, true)
                .await
                .map_err(parse_hdfs_error)?;
        } else {
            self.client
                .create(&to_path, WriteOptions::default().create_parent(true))
                .await
                .map_err(parse_hdfs_error)?;
        }

        self.client
            .rename(&from_path, &to_path, true)
            .await
            .map_err(parse_hdfs_error)?;

        Ok(())
    }
}

/// Parse hdfs-native error into opendal::Error.
pub fn parse_hdfs_error(hdfs_error: HdfsError) -> Error {
    let (kind, retryable, msg) = match &hdfs_error {
        HdfsError::IOError(err) => (ErrorKind::Unexpected, false, err.to_string()),
        HdfsError::DataTransferError(msg) => (ErrorKind::Unexpected, false, msg.clone()),
        HdfsError::ChecksumError => (
            ErrorKind::Unexpected,
            false,
            "checksums didn't match".to_string(),
        ),
        HdfsError::UrlParseError(err) => (ErrorKind::Unexpected, false, err.to_string()),
        HdfsError::AlreadyExists(msg) => (ErrorKind::AlreadyExists, false, msg.clone()),
        HdfsError::OperationFailed(msg) => (ErrorKind::Unexpected, false, msg.clone()),
        HdfsError::RPCError(msg0, msg1) => {
            if msg0.contains("java.io.FileNotFoundException") {
                (ErrorKind::NotFound, false, msg1.clone())
            } else {
                (ErrorKind::Unexpected, false, msg1.clone())
            }
        }
        HdfsError::FileNotFound(msg) => (ErrorKind::NotFound, false, msg.clone()),
        HdfsError::BlocksNotFound(msg) => (ErrorKind::NotFound, false, msg.clone()),
        HdfsError::IsADirectoryError(msg) => (ErrorKind::IsADirectory, false, msg.clone()),
        _ => (
            ErrorKind::Unexpected,
            false,
            "unexpected error from hdfs".to_string(),
        ),
    };

    let mut err = Error::new(kind, msg).set_source(hdfs_error);

    if retryable {
        err = err.set_temporary();
    }

    err
}
