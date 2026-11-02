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
use std::io;
use std::sync::Arc;

use opendal_core::raw::*;
use opendal_core::*;

fn map_hdfs_rename_error(err: io::Error, if_not_exists: bool, to_path: &str) -> Error {
    if if_not_exists && err.kind() == io::ErrorKind::AlreadyExists {
        return Error::new(
            ErrorKind::ConditionNotMatch,
            "target path already exists while if_not_exists is set",
        )
        .with_context("to", to_path)
        .set_source(err);
    }

    new_std_io_error(err)
}

/// HdfsCore contains code that directly interacts with HDFS.
#[derive(Clone)]
pub struct HdfsCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub root: String,
    pub atomic_write_dir: Option<String>,
    pub client: Arc<hdrs::Client>,
}

impl Debug for HdfsCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdfsCore")
            .field("root", &self.root)
            .field("atomic_write_dir", &self.atomic_write_dir)
            .finish_non_exhaustive()
    }
}

impl HdfsCore {
    pub fn hdfs_create_dir(&self, path: &str) -> Result<()> {
        let p = build_rooted_abs_path(&self.root, path);
        self.client.create_dir(&p).map_err(new_std_io_error)?;
        Ok(())
    }

    pub fn hdfs_stat(&self, path: &str) -> Result<Metadata> {
        let p = build_rooted_abs_path(&self.root, path);
        let meta = self.client.metadata(&p).map_err(new_std_io_error)?;

        let mode = if meta.is_dir() {
            EntryMode::DIR
        } else if meta.is_file() {
            EntryMode::FILE
        } else {
            EntryMode::Unknown
        };
        let mut m = match mode {
            EntryMode::FILE => MetadataBuilder::file(meta.len()),
            EntryMode::DIR => MetadataBuilder::dir(),
            EntryMode::Unknown => MetadataBuilder::unknown(),
        };
        m.last_modified(Timestamp::try_from(meta.modified())?);

        Ok(m.build())
    }

    pub async fn hdfs_open(&self, path: &str) -> Result<hdrs::File> {
        let p = build_rooted_abs_path(&self.root, path);

        let client = self.client.clone();
        let f = tokio::task::spawn_blocking(move || client.open_file().read(true).open(&p))
            .await
            .map_err(|e| Error::new(ErrorKind::Unexpected, "tokio task join failed").set_source(e))?
            .map_err(new_std_io_error)?;

        Ok(f)
    }

    pub async fn hdfs_write(
        &self,
        path: &str,
        op: &OpWrite,
    ) -> Result<(String, Option<String>, hdrs::AsyncFile, bool, u64)> {
        let target_path = build_rooted_abs_path(&self.root, path);
        let mut initial_size = 0;
        let target_exists = match self.client.metadata(&target_path) {
            Ok(meta) => {
                initial_size = meta.len();
                true
            }
            Err(err) => {
                if err.kind() != io::ErrorKind::NotFound {
                    return Err(new_std_io_error(err));
                }
                false
            }
        };

        let should_append = op.append() && target_exists;
        let tmp_path = self.atomic_write_dir.as_ref().and_then(|atomic_write_dir| {
            // If the target file exists, we should append to the end of it directly.
            (!should_append).then_some(build_rooted_abs_path(
                atomic_write_dir,
                &build_tmp_path_of(path),
            ))
        });

        if !target_exists {
            self.ensure_parent_dir(&target_path)?;
        }
        if !should_append {
            initial_size = 0;
        }

        let mut open_options = self.client.open_file();
        open_options.create(true);
        if should_append {
            open_options.append(true);
        } else {
            open_options.write(true);
        }

        let f = open_options
            .async_open(tmp_path.as_ref().unwrap_or(&target_path))
            .await
            .map_err(new_std_io_error)?;

        Ok((target_path, tmp_path, f, target_exists, initial_size))
    }

    pub fn hdfs_list(&self, path: &str) -> Result<Option<hdrs::Readdir>> {
        let p = build_rooted_abs_path(&self.root, path);

        match self.client.read_dir(&p) {
            Ok(f) => Ok(Some(f)),
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(new_std_io_error(e))
                }
            }
        }
    }

    pub fn hdfs_rename(&self, from: &str, to: &str, args: &OpRename) -> Result<()> {
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

                self.ensure_parent_dir(&to_path)?;
            }
            Ok(metadata) => {
                if metadata.is_file() {
                    if args.if_not_exists() {
                        return Err(Error::new(
                            ErrorKind::ConditionNotMatch,
                            "target path already exists while if_not_exists is set",
                        )
                        .with_context("to", &to_path));
                    }
                    self.client
                        .remove_file(&to_path)
                        .map_err(new_std_io_error)?;
                } else {
                    return Err(Error::new(ErrorKind::IsADirectory, "path should be a file")
                        .with_context("to", &to_path));
                }
            }
        }

        self.client
            .rename_file(&from_path, &to_path)
            .map_err(|err| map_hdfs_rename_error(err, args.if_not_exists(), &to_path))?;

        Ok(())
    }

    pub async fn hdfs_copy(&self, from: &str, to: &str) -> Result<Metadata> {
        let from_path = build_rooted_abs_path(&self.root, from);
        // OpenDAL copy is file-to-file only. Reject directory sources before
        // the HDFS API can recursively copy their contents.
        let from_meta = self.client.metadata(&from_path).map_err(new_std_io_error)?;
        if !from_meta.is_file() {
            return Err(
                Error::new(ErrorKind::IsADirectory, "from path should be a file")
                    .with_context("from", &from_path),
            );
        }

        let to_path = build_rooted_abs_path(&self.root, to);
        match self.client.metadata(&to_path) {
            Ok(meta) => {
                // OpenDAL treats `to` as the exact destination file path. Reject
                // an existing directory instead of copying the source into it.
                if meta.is_dir() {
                    return Err(
                        Error::new(ErrorKind::IsADirectory, "to path should be a file")
                            .with_context("to", &to_path),
                    );
                }
                // The HDFS copy API does not replace an existing destination,
                // so remove it first to preserve OpenDAL's overwrite semantics.
                self.client
                    .remove_file(&to_path)
                    .map_err(new_std_io_error)?;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                // hdrs copy_file requires the destination parent to already exist.
                self.ensure_parent_dir(&to_path)?;
            }
            Err(err) => return Err(new_std_io_error(err)),
        }

        let client = self.client.clone();
        let copy_from = from_path.clone();
        let copy_to = to_path.clone();
        tokio::task::spawn_blocking(move || client.copy_file(&copy_from, &copy_to))
            .await
            .map_err(|e| Error::new(ErrorKind::Unexpected, "tokio task join failed").set_source(e))?
            .map_err(new_std_io_error)?;

        Ok(MetadataBuilder::file(from_meta.len()).build())
    }

    fn ensure_parent_dir(&self, path: &str) -> Result<()> {
        self.client
            .create_dir(get_parent(path))
            .map_err(new_std_io_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_existing_target_error_to_condition_not_match() {
        let err = map_hdfs_rename_error(
            io::Error::new(io::ErrorKind::AlreadyExists, "target exists"),
            true,
            "/target",
        );

        assert_eq!(err.kind(), ErrorKind::ConditionNotMatch);
    }
}
