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

use super::core::{BucketOperation, DeletedFile, DeletedFolder, HfCore};
use opendal_core::raw::oio::BatchDeleteResult;
use opendal_core::raw::*;
use opendal_core::*;

pub struct HfDeleter {
    core: Arc<HfCore>,
    ctx: OperationContext,
}

impl HfDeleter {
    pub fn new(core: Arc<HfCore>, ctx: OperationContext) -> Self {
        Self { core, ctx }
    }

    async fn delete_paths(&self, paths: Vec<String>) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let result = if self.core.repo.is_bucket() {
            let ops = paths
                .into_iter()
                .map(|path| BucketOperation::DeleteFile { path })
                .collect();
            self.core.commit_bucket(&self.ctx, ops).await.map(|_| ())
        } else {
            let mut deleted_files = Vec::new();
            let mut deleted_folders = Vec::new();
            for path in paths {
                if path.ends_with('/') {
                    deleted_folders.push(DeletedFolder { path });
                } else {
                    deleted_files.push(DeletedFile { path });
                }
            }
            self.core
                .commit_git(&self.ctx, vec![], vec![], deleted_files, deleted_folders)
                .await
                .map(|_| ())
        };

        match result {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }
}

impl oio::BatchDelete for HfDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let repo_path = self.core.repo_path(&path);
        self.delete_paths(vec![repo_path]).await
    }

    async fn delete_batch(&self, batch: Vec<(String, OpDelete)>) -> Result<BatchDeleteResult> {
        let paths: Vec<String> = batch
            .iter()
            .map(|(path, _)| self.core.repo_path(path))
            .collect();

        self.delete_paths(paths).await?;
        Ok(BatchDeleteResult {
            succeeded: batch,
            failed: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::super::core::HfRepoType;
    use super::super::core::test_utils::create_test_core;
    use super::*;
    use opendal_core::raw::oio::BatchDelete;

    #[tokio::test]
    async fn test_git_delete_separates_files_and_folders() -> Result<()> {
        let (mut core, ctx, mock_client) = create_test_core(
            HfRepoType::Dataset,
            "test-org/test-dataset",
            "main",
            "https://huggingface.co",
        );
        core.token = Some("test-token".to_string());

        let deleter = HfDeleter::new(Arc::new(core), ctx);
        let batch = vec![
            ("dir/".to_string(), OpDelete::default()),
            ("dir/a".to_string(), OpDelete::default()),
            ("dir/b/".to_string(), OpDelete::default()),
            ("dir/b/c".to_string(), OpDelete::default()),
        ];

        deleter.delete_batch(batch).await?;

        let body: Value = serde_json::from_str(&mock_client.get_captured_body())
            .expect("commit payload must be valid json");
        let folders: Vec<&str> = body["deletedFolders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["path"].as_str().unwrap())
            .collect();
        assert!(folders.contains(&"dir/"));
        assert!(folders.contains(&"dir/b/"));

        let files: Vec<&str> = body["deletedFiles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["path"].as_str().unwrap())
            .collect();
        assert!(files.contains(&"dir/a"));
        assert!(files.contains(&"dir/b/c"));

        Ok(())
    }
}
