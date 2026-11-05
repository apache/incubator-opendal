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

use std::sync::Arc;

use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub struct FsDeleter {
    core: Arc<FsCore>,
}

impl FsDeleter {
    pub fn new(core: Arc<FsCore>) -> Self {
        Self { core }
    }
}

impl oio::OneShotDelete for FsDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        let p = self.core.root_join(&path)?;

        let recursive = args.recursive();

        // Use symlink_metadata so symlinks are never followed: a dangling
        // symlink must be removed itself instead of reported as already
        // deleted, and a symlink to a directory must be removed itself
        // instead of failing inside remove_dir.
        let meta = tokio::fs::symlink_metadata(&p).await;

        match meta {
            Ok(meta) => {
                if meta.is_dir() {
                    if recursive {
                        tokio::fs::remove_dir_all(&p)
                            .await
                            .map_err(new_std_io_error)?;
                    } else {
                        tokio::fs::remove_dir(&p).await.map_err(new_std_io_error)?;
                    }
                } else {
                    tokio::fs::remove_file(&p).await.map_err(new_std_io_error)?;
                }
                Ok(())
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(new_std_io_error(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[tokio::test]
    async fn test_delete_dangling_symlink_removes_link() {
        use opendal_core::Operator;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path();

        std::os::unix::fs::symlink("/nonexistent-target", root.join("dangle")).unwrap();

        let op = Operator::new(crate::Fs::default().root(root.to_str().unwrap())).unwrap();
        op.delete("dangle").await.unwrap();
        assert!(std::fs::symlink_metadata(root.join("dangle")).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_delete_symlink_to_dir_removes_link_not_target() {
        use opendal_core::Operator;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::create_dir(root.join("targetdir")).unwrap();
        std::fs::write(root.join("targetdir").join("keep.txt"), b"keep").unwrap();
        std::os::unix::fs::symlink(root.join("targetdir"), root.join("linkdir")).unwrap();

        let op = Operator::new(crate::Fs::default().root(root.to_str().unwrap())).unwrap();
        op.delete("linkdir").await.unwrap();
        assert!(std::fs::symlink_metadata(root.join("linkdir")).is_err());
        assert_eq!(
            std::fs::read(root.join("targetdir").join("keep.txt")).unwrap(),
            b"keep"
        );

        // Recursive delete of a symlink to a dir removes the link, not the
        // target (rm semantics).
        std::os::unix::fs::symlink(root.join("targetdir"), root.join("linkdir2")).unwrap();
        op.delete_with("linkdir2").recursive(true).await.unwrap();
        assert!(std::fs::symlink_metadata(root.join("linkdir2")).is_err());
        assert_eq!(
            std::fs::read(root.join("targetdir").join("keep.txt")).unwrap(),
            b"keep"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_delete_symlink_to_file_removes_link_not_target() {
        use opendal_core::Operator;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(root.join("real.txt"), b"data").unwrap();
        std::os::unix::fs::symlink(root.join("real.txt"), root.join("linkfile")).unwrap();

        let op = Operator::new(crate::Fs::default().root(root.to_str().unwrap())).unwrap();
        op.delete("linkfile").await.unwrap();
        assert!(std::fs::symlink_metadata(root.join("linkfile")).is_err());
        assert_eq!(std::fs::read(root.join("real.txt")).unwrap(), b"data");
    }
}
