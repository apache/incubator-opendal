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
use crate::raw::oio;
use crate::raw::*;
use crate::*;

pub struct MemoryDeleter {
    core: Arc<MemoryCore>,
    root: String,
}

impl MemoryDeleter {
    pub fn new(core: Arc<MemoryCore>, root: String) -> Self {
        Self { core, root }
    }
}

impl oio::OneShotDelete for MemoryDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        let p = build_abs_path(&self.root, &path);

        // An exact key always wins: deleting a file removes it, even if
        // other keys happen to share its prefix.
        if self.core.get(&p)?.is_some() {
            self.core.delete(&p)?;
            return Ok(());
        }

        // No exact key: the path may still denote a non-empty directory,
        // since memory synthesizes directory entries from key prefixes.
        // Mirror the fs service here: removing a non-empty directory
        // without `recursive` is an error instead of a silent success,
        // while deleting an empty or missing directory stays a no-op.
        let prefix = if p.is_empty() {
            String::new()
        } else if p.ends_with('/') {
            p.clone()
        } else {
            format!("{p}/")
        };
        let children = self.core.scan(&prefix)?;
        if children.is_empty() {
            return Ok(());
        }
        if args.recursive() {
            for key in children {
                self.core.delete(&key)?;
            }
            return Ok(());
        }
        Err(Error::new(ErrorKind::Unexpected, "directory not empty"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::oio::OneShotDelete;

    fn put(core: &MemoryCore, key: &str) {
        core.set(
            key,
            MemoryValue {
                metadata: MetadataBuilder::file(1).build(),
                content: Buffer::from(vec![b'x']),
            },
        )
        .unwrap();
    }

    fn op_delete(recursive: bool) -> OpDelete {
        OpDelete::from_options(
            &Capability::default(),
            options::DeleteOptions {
                recursive,
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn deleter(core: Arc<MemoryCore>) -> MemoryDeleter {
        MemoryDeleter::new(core, "/".to_string())
    }

    #[tokio::test]
    async fn test_delete_non_empty_dir_fails_and_keeps_children() {
        for path in ["d/", "d"] {
            let core = Arc::new(MemoryCore::new());
            put(&core, "d/a.txt");
            put(&core, "d/b.txt");

            let err = deleter(core.clone())
                .delete_once(path.to_string(), op_delete(false))
                .await
                .expect_err("deleting a non-empty directory must fail");
            assert_eq!(err.kind(), ErrorKind::Unexpected);
            assert!(err.to_string().contains("directory not empty"));
            assert!(core.get("d/a.txt").unwrap().is_some());
            assert!(core.get("d/b.txt").unwrap().is_some());
        }
    }

    #[tokio::test]
    async fn test_delete_empty_and_missing_dir_succeeds() {
        let core = Arc::new(MemoryCore::new());
        deleter(core.clone())
            .delete_once("ghost/".to_string(), op_delete(false))
            .await
            .expect("deleting a missing directory must succeed");
        deleter(core)
            .delete_once("ghost".to_string(), op_delete(false))
            .await
            .expect("deleting a missing directory must succeed");
    }

    #[tokio::test]
    async fn test_delete_recursive_removes_subtree() {
        let core = Arc::new(MemoryCore::new());
        put(&core, "d/a.txt");
        put(&core, "d/b.txt");
        put(&core, "other.txt");

        deleter(core.clone())
            .delete_once("d/".to_string(), op_delete(true))
            .await
            .expect("recursive delete must succeed");
        assert!(core.get("d/a.txt").unwrap().is_none());
        assert!(core.get("d/b.txt").unwrap().is_none());
        assert!(core.get("other.txt").unwrap().is_some());
    }

    #[tokio::test]
    async fn test_delete_file_still_works() {
        let core = Arc::new(MemoryCore::new());
        put(&core, "f.txt");

        deleter(core.clone())
            .delete_once("f.txt".to_string(), op_delete(false))
            .await
            .expect("deleting a file must succeed");
        assert!(core.get("f.txt").unwrap().is_none());

        deleter(core)
            .delete_once("f.txt".to_string(), op_delete(false))
            .await
            .expect("deleting a missing file must succeed");
    }
}
