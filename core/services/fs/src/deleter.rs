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

        let meta = tokio::fs::metadata(&p).await;

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
                    // A path ending with `/` denotes a directory. Never remove
                    // a file through a directory path: `confined_join` strips
                    // the trailing slash, so without this check deleting
                    // `f.txt/` would remove the file `f.txt`.
                    if path.ends_with('/') {
                        return Err(Error::new(
                            ErrorKind::NotADirectory,
                            "delete path is a directory path, but the target is a file",
                        ));
                    }
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
    use super::*;

    #[tokio::test]
    async fn test_delete_file_through_directory_path_is_rejected() {
        use opendal_core::Operator;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path();

        let op = Operator::new(crate::Fs::default().root(root.to_str().unwrap())).unwrap();

        op.write("f.txt", "x").await.unwrap();
        let err = op.delete("f.txt/").await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotADirectory);
        // The file must survive: a directory path must never remove a file.
        assert!(op.stat("f.txt").await.is_ok());

        // Directory deletes keep working.
        op.create_dir("d/").await.unwrap();
        op.delete("d/").await.unwrap();
        assert!(op.stat("d/").await.is_err());
    }
}
