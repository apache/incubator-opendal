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

use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use bytes::Buf;
use tokio::io::AsyncWriteExt;

use super::core::FsCore;
use opendal_core::raw::*;
use opendal_core::*;

pub type FsWriters = TwoWays<FsWriter, oio::PositionWriter<FsWriter>>;

pub struct FsWriter {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    root: PathBuf,
    target_path: PathBuf,
    /// The temp_path is used to specify whether we should move to target_path after the file has been closed.
    temp_path: Option<PathBuf>,
    f: tokio::fs::File,
    /// User metadata to be written to xattr on Unix systems.
    #[cfg(unix)]
    user_metadata: Option<HashMap<String, String>>,
}

impl FsWriter {
    pub async fn create(core: Arc<FsCore>, path: &str, op: OpWrite) -> Result<Self> {
        let target_path = core.ensure_write_abs_path(&core.root, path).await?;

        // Store user metadata for later use on Unix systems.
        #[cfg(unix)]
        let user_metadata = op.user_metadata().map(|metadata| {
            metadata
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect()
        });

        // Quick path while atomic_write_dir is not set.
        if core.atomic_write_dir.is_none() {
            let target_file = core.fs_write(&target_path, &op).await?;

            return Ok(Self {
                #[cfg(any(
                    target_os = "linux",
                    target_os = "android",
                    target_vendor = "apple",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                    target_os = "dragonfly"
                ))]
                root: core.root.clone(),
                target_path,
                temp_path: None,
                f: target_file,
                #[cfg(unix)]
                user_metadata,
            });
        }

        let is_append = op.append();
        let is_exist = tokio::fs::try_exists(&target_path)
            .await
            .map_err(new_std_io_error)?;
        if op.if_not_exists() && is_exist {
            return Err(Error::new(
                ErrorKind::ConditionNotMatch,
                "file already exists, doesn't match the condition if_not_exists",
            ));
        }

        // The only case we allow write in place is the file
        // exists and users request for append writing.
        let (f, temp_path) = if !(is_append && is_exist) {
            core.fs_tempfile_write(path).await?
        } else {
            let f = core.fs_write(&target_path, &op).await?;
            (f, None)
        };

        Ok(Self {
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "dragonfly"
            ))]
            root: core.root.clone(),
            target_path,
            temp_path,
            f,
            #[cfg(unix)]
            user_metadata,
        })
    }

    async fn finish(&self) -> Result<std::fs::Metadata> {
        let file = self
            .f
            .try_clone()
            .await
            .map_err(new_std_io_error)?
            .into_std()
            .await;

        let target_path = self.target_path.clone();
        let temp_path = self.temp_path.clone();

        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "dragonfly"
        ))]
        let root = self.root.clone();
        #[cfg(unix)]
        let user_metadata = self.user_metadata.clone();

        tokio::task::spawn_blocking(move || {
            #[cfg(unix)]
            if let Some(user_metadata) = user_metadata {
                use xattr::FileExt;

                // Set attributes on the inode being written before publishing it.
                for (key, value) in user_metadata {
                    file.set_xattr(format!("user.{key}"), value.as_bytes())
                        .map_err(new_std_io_error)?;
                }
            }

            file.sync_all()
                .map_err(|err| new_std_io_error(err).set_permanent())?;

            let metadata = file.metadata().map_err(new_std_io_error)?;

            if let Some(temp_path) = &temp_path {
                std::fs::rename(temp_path, &target_path).map_err(new_std_io_error)?;

                #[cfg(any(
                    target_os = "linux",
                    target_os = "android",
                    target_vendor = "apple",
                    target_os = "freebsd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                    target_os = "dragonfly"
                ))]
                {
                    for parent in target_path.ancestors().skip(1) {
                        File::open(parent)
                            .and_then(|dir| dir.sync_all())
                            .map_err(|err| new_std_io_error(err).set_permanent())?;
                        if parent == root {
                            break;
                        }
                    }

                    if let Some(parent) = temp_path.parent()
                        && Some(parent) != target_path.parent()
                    {
                        File::open(parent)
                            .and_then(|dir| dir.sync_all())
                            .map_err(|err| new_std_io_error(err).set_permanent())?;
                    }
                }
            }

            Ok(metadata)
        })
        .await
        .map_err(new_task_join_error)?
    }
}

/// # Safety
///
/// We will only take `&mut Self` reference for FsWriter.
unsafe impl Sync for FsWriter {}

impl oio::Write for FsWriter {
    async fn write(&mut self, mut bs: Buffer) -> Result<()> {
        while bs.has_remaining() {
            let n = self.f.write(bs.chunk()).await.map_err(new_std_io_error)?;
            bs.advance(n);
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.f.flush().await.map_err(new_std_io_error)?;
        let file_meta = self.finish().await?;
        let mut meta = MetadataBuilder::file(file_meta.len());
        meta.last_modified(Timestamp::try_from(
            file_meta.modified().map_err(new_std_io_error)?,
        )?);
        Ok(meta.build())
    }

    async fn abort(&mut self) -> Result<()> {
        if let Some(temp_path) = &self.temp_path {
            tokio::fs::remove_file(temp_path)
                .await
                .map_err(new_std_io_error)
        } else {
            Err(Error::new(
                ErrorKind::Unsupported,
                "Fs doesn't support abort if atomic_write_dir is not set",
            ))
        }
    }
}

impl oio::PositionWrite for FsWriter {
    async fn write_all_at(&self, offset: u64, buf: Buffer) -> Result<()> {
        let f = self
            .f
            .try_clone()
            .await
            .map_err(new_std_io_error)?
            .into_std()
            .await;

        tokio::task::spawn_blocking(move || {
            let mut buf = buf;
            let mut offset = offset;
            while !buf.is_empty() {
                match write_at(&f, buf.chunk(), offset) {
                    Ok(n) => {
                        buf.advance(n);
                        offset += n as u64
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        })
        .await
        .map_err(new_task_join_error)?
    }

    async fn close(&self, _size: u64) -> Result<Metadata> {
        let file_meta = self.finish().await?;
        let mode = if file_meta.is_file() {
            EntryMode::FILE
        } else if file_meta.is_dir() {
            EntryMode::DIR
        } else {
            EntryMode::Unknown
        };
        let mut meta = match mode {
            EntryMode::FILE => MetadataBuilder::file(file_meta.len()),
            EntryMode::DIR => MetadataBuilder::dir(),
            EntryMode::Unknown => MetadataBuilder::unknown(),
        };
        meta.last_modified(Timestamp::try_from(
            file_meta.modified().map_err(new_std_io_error)?,
        )?);
        Ok(meta.build())
    }

    async fn abort(&self) -> Result<()> {
        if let Some(temp_path) = &self.temp_path {
            tokio::fs::remove_file(temp_path)
                .await
                .map_err(new_std_io_error)
        } else {
            Err(Error::new(
                ErrorKind::Unsupported,
                "Fs doesn't support abort if atomic_write_dir is not set",
            ))
        }
    }
}

#[cfg(windows)]
fn write_at(f: &File, buf: &[u8], offset: u64) -> Result<usize> {
    use std::os::windows::fs::FileExt;
    f.seek_write(buf, offset).map_err(new_std_io_error)
}

#[cfg(unix)]
fn write_at(f: &File, buf: &[u8], offset: u64) -> Result<usize> {
    use std::os::unix::fs::FileExt;
    f.write_at(buf, offset).map_err(new_std_io_error)
}
