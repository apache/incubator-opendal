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
use std::fmt::Formatter;
use std::sync::Arc;

use hdfs_native::WriteOptions;
use log::debug;

use super::delete::HdfsNativeDeleter;
use super::error::parse_hdfs_error;
use super::lister::HdfsNativeLister;
use super::reader::HdfsNativeReader;
use super::writer::HdfsNativeWriter;
use crate::raw::*;
use crate::services::HdfsNativeConfig;
use crate::*;

/// [Hadoop Distributed File System (HDFS™)](https://hadoop.apache.org/) support.
/// Using [Native Rust HDFS client](https://github.com/Kimahriman/hdfs-native).
impl Configurator for HdfsNativeConfig {
    type Builder = HdfsNativeBuilder;
    fn into_builder(self) -> Self::Builder {
        HdfsNativeBuilder { config: self }
    }
}

#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct HdfsNativeBuilder {
    config: HdfsNativeConfig,
}

impl Debug for HdfsNativeBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdfsNativeBuilder")
            .field("config", &self.config)
            .finish()
    }
}

impl HdfsNativeBuilder {
    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set url of this backend.
    ///
    /// Valid format including:
    ///
    /// - `default`: using the default setting based on hadoop config.
    /// - `hdfs://127.0.0.1:9000`: connect to hdfs cluster.
    pub fn url(mut self, url: &str) -> Self {
        if !url.is_empty() {
            // Trim trailing `/` so that we can accept `http://127.0.0.1:9000/`
            self.config.url = Some(url.trim_end_matches('/').to_string())
        }

        self
    }

    /// Enable append capacity of this backend.
    ///
    /// This should be disabled when HDFS runs in non-distributed mode.
    pub fn enable_append(mut self, enable_append: bool) -> Self {
        self.config.enable_append = enable_append;
        self
    }
}

impl Builder for HdfsNativeBuilder {
    const SCHEME: Scheme = Scheme::HdfsNative;
    type Config = HdfsNativeConfig;

    fn build(self) -> Result<impl Access> {
        debug!("backend build started: {:?}", &self);

        let url = match &self.config.url {
            Some(v) => v,
            None => {
                return Err(Error::new(ErrorKind::ConfigInvalid, "url is empty")
                    .with_context("service", Scheme::HdfsNative));
            }
        };

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {}", root);

        let client = hdfs_native::Client::new(url).map_err(parse_hdfs_error)?;

        // need to check if root dir exists, create if not

        Ok(HdfsNativeBackend {
            root,
            client: Arc::new(client),
            _enable_append: self.config.enable_append,
        })
    }
}

// #[inline]
// fn tmp_file_of(path: &str) -> String {
//     let name = get_basename(path);
//     let uuid = Uuid::new_v4().to_string();
//
//     format!("{name}.{uuid}")
// }

/// Backend for hdfs-native services.
#[derive(Debug, Clone)]
pub struct HdfsNativeBackend {
    pub root: String,
    pub client: Arc<hdfs_native::Client>,
    _enable_append: bool,
}

/// hdfs_native::Client is thread-safe.
unsafe impl Send for HdfsNativeBackend {}
unsafe impl Sync for HdfsNativeBackend {}

impl Access for HdfsNativeBackend {
    type Reader = HdfsNativeReader;
    type Writer = HdfsNativeWriter;
    type Lister = Option<HdfsNativeLister>;
    type Deleter = oio::OneShotDeleter<HdfsNativeDeleter>;
    type BlockingReader = ();
    type BlockingWriter = ();
    type BlockingLister = ();
    type BlockingDeleter = ();

    fn info(&self) -> AccessorInfo {
        let mut am = AccessorInfo::default();
        am.set_scheme(Scheme::HdfsNative)
            .set_root(&self.root)
            .set_native_capability(Capability {
                stat: true,
                list: true,
                read: true,
                write: true,
                stat_has_last_modified: true,
                stat_has_content_length: true,

                delete: true,
                rename: true,
                blocking: true,

                ..Default::default()
            });

        am
    }

    async fn create_dir(&self, path: &str, _args: OpCreateDir) -> Result<RpCreateDir> {
        let p = build_rooted_abs_path(&self.root, path);

        self.client
            .mkdirs(&p, 0o777, true)
            .await
            .map_err(parse_hdfs_error)?;
        Ok(RpCreateDir::default())
    }

    async fn stat(&self, path: &str, _args: OpStat) -> Result<RpStat> {
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

        let mut metadata = Metadata::new(mode);
        metadata
            .set_last_modified(parse_datetime_from_from_timestamp_millis(
                status.modification_time as i64,
            )?)
            .set_content_length(status.length as u64);

        Ok(RpStat::new(metadata))
    }

    async fn read(&self, path: &str, _args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let p = build_rooted_abs_path(&self.root, path);

        let f = self.client.read(&p).await.map_err(parse_hdfs_error)?;

        let r = HdfsNativeReader::new(f);

        Ok((RpRead::new(), r))
    }

    async fn write(&self, path: &str, _args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let p = build_rooted_abs_path(&self.root, path);

        let f = self
            .client
            .create(&p, WriteOptions::default())
            .await
            .map_err(parse_hdfs_error)?;

        let w = HdfsNativeWriter::new(f);

        Ok((RpWrite::new(), w))
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        Ok((
            RpDelete::default(),
            oio::OneShotDeleter::new(HdfsNativeDeleter::new(Arc::new(self.clone()))),
        ))
    }

    async fn list(&self, path: &str, _args: OpList) -> Result<(RpList, Self::Lister)> {
        let p = build_rooted_abs_path(&self.root, path);
        let l = HdfsNativeLister::new(p, self.client.clone());
        Ok((RpList::default(), Some(l)))
    }

    async fn rename(&self, from: &str, to: &str, _args: OpRename) -> Result<RpRename> {
        let from_path = build_rooted_abs_path(&self.root, from);
        let to_path = build_rooted_abs_path(&self.root, to);

        self.client
            .rename(&from_path, &to_path, false)
            .await
            .map_err(parse_hdfs_error)?;

        Ok(RpRename::default())
    }
}
