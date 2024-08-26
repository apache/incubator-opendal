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
use std::io;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;

use futures::AsyncWriteExt;
use log::debug;
use uuid::Uuid;

use super::lister::HdfsLister;
use super::writer::HdfsWriter;
use crate::raw::*;
use crate::services::hdfs::reader::HdfsReader;
use crate::services::HdfsConfig;
use crate::*;

impl Configurator for HdfsConfig {
    type Builder = HdfsBuilder;
    fn into_builder(self) -> Self::Builder {
        HdfsBuilder { config: self }
    }
}

#[doc = include_str!("docs.md")]
#[derive(Default)]
pub struct HdfsBuilder {
    config: HdfsConfig,
}

impl Debug for HdfsBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdfsBuilder")
            .field("config", &self.config)
            .finish()
    }
}

impl HdfsBuilder {
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

    /// Set name_node of this backend.
    ///
    /// Valid format including:
    ///
    /// - `default`: using the default setting based on hadoop config.
    /// - `hdfs://127.0.0.1:9000`: connect to hdfs cluster.
    pub fn name_node(mut self, name_node: &str) -> Self {
        if !name_node.is_empty() {
            // Trim trailing `/` so that we can accept `http://127.0.0.1:9000/`
            self.config.name_node = Some(name_node.trim_end_matches('/').to_string())
        }

        self
    }

    /// Set kerberos_ticket_cache_path of this backend
    ///
    /// This should be configured when kerberos is enabled.
    pub fn kerberos_ticket_cache_path(mut self, kerberos_ticket_cache_path: &str) -> Self {
        if !kerberos_ticket_cache_path.is_empty() {
            self.config.kerberos_ticket_cache_path = Some(kerberos_ticket_cache_path.to_string())
        }
        self
    }

    /// Set user of this backend
    pub fn user(mut self, user: &str) -> Self {
        if !user.is_empty() {
            self.config.user = Some(user.to_string())
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

    /// Set temp dir for atomic write.
    ///
    /// # Notes
    ///
    /// - When append is enabled, we will not use atomic write
    ///   to avoid data loss and performance issue.
    pub fn atomic_write_dir(mut self, dir: &str) -> Self {
        self.config.atomic_write_dir = if dir.is_empty() {
            None
        } else {
            Some(String::from(dir))
        };
        self
    }
}

impl Builder for HdfsBuilder {
    const SCHEME: Scheme = Scheme::Hdfs;
    type Config = HdfsConfig;

    fn build(self) -> Result<impl Access> {
        debug!("backend build started: {:?}", &self);

        let name_node = match &self.config.name_node {
            Some(v) => v,
            None => {
                return Err(Error::new(ErrorKind::ConfigInvalid, "name node is empty")
                    .with_context("service", Scheme::Hdfs))
            }
        };

        let root = normalize_root(&self.config.root.unwrap_or_default());
        debug!("backend use root {}", root);

        let mut builder = hdrs::ClientBuilder::new(name_node);
        if let Some(ticket_cache_path) = &self.config.kerberos_ticket_cache_path {
            builder = builder.with_kerberos_ticket_cache_path(ticket_cache_path.as_str());
        }
        if let Some(user) = &self.config.user {
            builder = builder.with_user(user.as_str());
        }

        let client = builder.connect().map_err(new_std_io_error)?;

        // Create root dir if not exist.
        if let Err(e) = client.metadata(&root) {
            if e.kind() == io::ErrorKind::NotFound {
                debug!("root {} is not exist, creating now", root);

                client.create_dir(&root).map_err(new_std_io_error)?
            }
        }

        let atomic_write_dir = self.config.atomic_write_dir;

        // If atomic write dir is not exist, we must create it.
        if let Some(d) = &atomic_write_dir {
            if let Err(e) = client.metadata(d) {
                if e.kind() == io::ErrorKind::NotFound {
                    client.create_dir(d).map_err(new_std_io_error)?
                }
            }
        }

        Ok(HdfsBackend {
            root,
            atomic_write_dir,
            client: Arc::new(client),
            enable_append: self.config.enable_append,
        })
    }
}

#[inline]
fn tmp_file_of(path: &str) -> String {
    let name = get_basename(path);
    let uuid = Uuid::new_v4().to_string();

    format!("{name}.{uuid}")
}

/// Backend for hdfs services.
#[derive(Debug, Clone)]
pub struct HdfsBackend {
    root: String,
    atomic_write_dir: Option<String>,
    client: Arc<hdrs::Client>,
    enable_append: bool,
}

/// hdrs::Client is thread-safe.
unsafe impl Send for HdfsBackend {}
unsafe impl Sync for HdfsBackend {}

impl Access for HdfsBackend {
    type Reader = HdfsReader<hdrs::AsyncFile>;
    type Writer = HdfsWriter<hdrs::AsyncFile>;
    type Lister = Option<HdfsLister>;
    type BlockingReader = HdfsReader<hdrs::File>;
    type BlockingWriter = HdfsWriter<hdrs::File>;
    type BlockingLister = Option<HdfsLister>;

    fn info(&self) -> Arc<AccessorInfo> {
        let mut am = AccessorInfo::default();
        am.set_scheme(Scheme::Hdfs)
            .set_root(&self.root)
            .set_native_capability(Capability {
                stat: true,

                read: true,

                write: true,
                write_can_append: self.enable_append,

                create_dir: true,
                delete: true,

                list: true,

                rename: true,
                blocking: true,

                ..Default::default()
            });

        am.into()
    }

    async fn create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        self.blocking_create_dir(path, args)
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        self.blocking_stat(path, args)
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let p = build_rooted_abs_path(&self.root, path);

        let client = self.client.clone();
        let mut f = client
            .open_file()
            .read(true)
            .async_open(&p)
            .await
            .map_err(new_std_io_error)?;

        if args.range().offset() != 0 {
            use futures::AsyncSeekExt;

            f.seek(SeekFrom::Start(args.range().offset()))
                .await
                .map_err(new_std_io_error)?;
        }

        Ok((
            RpRead::new(),
            HdfsReader::new(f, args.range().size().unwrap_or(u64::MAX) as _),
        ))
    }

    async fn write(&self, path: &str, op: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let (target_path, tmp_path) = if let Some(atomic_write_dir) = &self.atomic_write_dir {
            let target_path = build_rooted_abs_path(&self.root, path);
            let tmp_path = build_rooted_abs_path(atomic_write_dir, &tmp_file_of(path));

            // If the target file exists, we should append to the end of it directly.
            if op.append() && self.client.metadata(&target_path).is_ok() {
                (target_path, None)
            } else {
                (target_path, Some(tmp_path))
            }
        } else {
            let p = build_rooted_abs_path(&self.root, path);
            (p, None)
        };

        if let Err(err) = self.client.metadata(&target_path) {
            // Early return if other error happened.
            if err.kind() != io::ErrorKind::NotFound {
                return Err(new_std_io_error(err));
            }

            let parent = get_parent(&target_path);

            self.client.create_dir(parent).map_err(new_std_io_error)?;

            let mut f = self
                .client
                .open_file()
                .create(true)
                .write(true)
                .async_open(&target_path)
                .await
                .map_err(new_std_io_error)?;
            f.close().await.map_err(new_std_io_error)?;
        }

        let mut open_options = self.client.open_file();
        open_options.create(true);
        if op.append() {
            open_options.append(true);
        } else {
            open_options.write(true);
        }

        let f = open_options
            .async_open(tmp_path.as_ref().unwrap_or(&target_path))
            .await
            .map_err(new_std_io_error)?;

        Ok((
            RpWrite::new(),
            HdfsWriter::new(target_path, tmp_path, f, Arc::clone(&self.client)),
        ))
    }

    async fn delete(&self, path: &str, args: OpDelete) -> Result<RpDelete> {
        self.blocking_delete(path, args)
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        self.blocking_list(path, args)
    }

    async fn rename(&self, from: &str, to: &str, args: OpRename) -> Result<RpRename> {
        self.blocking_rename(from, to, args)
    }

    fn blocking_create_dir(&self, path: &str, _: OpCreateDir) -> Result<RpCreateDir> {
        let p = build_rooted_abs_path(&self.root, path);

        self.client.create_dir(&p).map_err(new_std_io_error)?;

        Ok(RpCreateDir::default())
    }

    fn blocking_stat(&self, path: &str, _: OpStat) -> Result<RpStat> {
        let p = build_rooted_abs_path(&self.root, path);

        let meta = self.client.metadata(&p).map_err(new_std_io_error)?;

        let mode = if meta.is_dir() {
            EntryMode::DIR
        } else if meta.is_file() {
            EntryMode::FILE
        } else {
            EntryMode::Unknown
        };
        let mut m = Metadata::new(mode);
        m.set_content_length(meta.len());
        m.set_last_modified(meta.modified().into());

        Ok(RpStat::new(m))
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        let p = build_rooted_abs_path(&self.root, path);

        let mut f = self
            .client
            .open_file()
            .read(true)
            .open(&p)
            .map_err(new_std_io_error)?;

        if args.range().offset() != 0 {
            use std::io::Seek;

            f.seek(SeekFrom::Start(args.range().offset()))
                .map_err(new_std_io_error)?;
        }

        Ok((
            RpRead::new(),
            HdfsReader::new(f, args.range().size().unwrap_or(u64::MAX) as _),
        ))
    }

    fn blocking_write(&self, path: &str, op: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        let (target_path, tmp_path) = if let Some(atomic_write_dir) = &self.atomic_write_dir {
            let target_path = build_rooted_abs_path(&self.root, path);
            let tmp_path = build_rooted_abs_path(atomic_write_dir, &tmp_file_of(path));

            // If the target file exists, we should append to the end of it directly.
            if op.append() && self.client.metadata(&target_path).is_ok() {
                (target_path, None)
            } else {
                (target_path, Some(tmp_path))
            }
        } else {
            let p = build_rooted_abs_path(&self.root, path);

            (p, None)
        };

        if let Err(err) = self.client.metadata(&target_path) {
            // Early return if other error happened.
            if err.kind() != io::ErrorKind::NotFound {
                return Err(new_std_io_error(err));
            }

            let parent = get_parent(&target_path);

            self.client.create_dir(parent).map_err(new_std_io_error)?;

            self.client
                .open_file()
                .create(true)
                .write(true)
                .open(&target_path)
                .map_err(new_std_io_error)?;
        }

        let mut open_options = self.client.open_file();
        open_options.create(true);
        if op.append() {
            open_options.append(true);
        } else {
            open_options.write(true);
        }

        let f = open_options
            .open(tmp_path.as_ref().unwrap_or(&target_path))
            .map_err(new_std_io_error)?;

        Ok((
            RpWrite::new(),
            HdfsWriter::new(target_path, tmp_path, f, Arc::clone(&self.client)),
        ))
    }

    fn blocking_delete(&self, path: &str, _: OpDelete) -> Result<RpDelete> {
        let p = build_rooted_abs_path(&self.root, path);

        let meta = self.client.metadata(&p);

        if let Err(err) = meta {
            return if err.kind() == io::ErrorKind::NotFound {
                Ok(RpDelete::default())
            } else {
                Err(new_std_io_error(err))
            };
        }

        // Safety: Err branch has been checked, it's OK to unwrap.
        let meta = meta.ok().unwrap();

        let result = if meta.is_dir() {
            self.client.remove_dir(&p)
        } else {
            self.client.remove_file(&p)
        };

        result.map_err(new_std_io_error)?;

        Ok(RpDelete::default())
    }

    fn blocking_list(&self, path: &str, _: OpList) -> Result<(RpList, Self::BlockingLister)> {
        let p = build_rooted_abs_path(&self.root, path);

        let f = match self.client.read_dir(&p) {
            Ok(f) => f,
            Err(e) => {
                return if e.kind() == io::ErrorKind::NotFound {
                    Ok((RpList::default(), None))
                } else {
                    Err(new_std_io_error(e))
                }
            }
        };

        let rd = HdfsLister::new(&self.root, f);

        Ok((RpList::default(), Some(rd)))
    }

    fn blocking_rename(&self, from: &str, to: &str, _args: OpRename) -> Result<RpRename> {
        let from_path = build_rooted_abs_path(&self.root, from);
        self.client.metadata(&from_path).map_err(new_std_io_error)?;

        let to_path = build_rooted_abs_path(&self.root, to);
        let result = self.client.metadata(&to_path);
        match result {
            Err(err) => {
                // Early return if other error happened.
                if err.kind() != io::ErrorKind::NotFound {
                    return Err(new_std_io_error(err));
                }

                let parent = PathBuf::from(&to_path)
                    .parent()
                    .ok_or_else(|| {
                        Error::new(
                            ErrorKind::Unexpected,
                            "path should have parent but not, it must be malformed",
                        )
                        .with_context("input", &to_path)
                    })?
                    .to_path_buf();

                self.client
                    .create_dir(&parent.to_string_lossy())
                    .map_err(new_std_io_error)?;
            }
            Ok(metadata) => {
                if metadata.is_file() {
                    self.client
                        .remove_file(&to_path)
                        .map_err(new_std_io_error)?;
                } else {
                    return Err(Error::new(ErrorKind::IsADirectory, "path should be a file")
                        .with_context("input", &to_path));
                }
            }
        }

        self.client
            .rename_file(&from_path, &to_path)
            .map_err(new_std_io_error)?;

        Ok(RpRename::new())
    }
}
