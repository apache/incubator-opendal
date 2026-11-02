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

use log::debug;

use super::GIT_SCHEME;
use super::config::GitConfig;
use super::core::GitCore;
use super::core::join_error;
use super::lister::GitLister;
use super::reader::GitReader;
use opendal_core::raw::*;
use opendal_core::*;

/// Git service support with optional transparent Git LFS resolution.
#[doc = include_str!("docs.md")]
#[derive(Debug, Default)]
pub struct GitBuilder {
    pub(super) config: GitConfig,
}

impl GitBuilder {
    /// Set the repository URL or local path.
    ///
    /// This is required.
    pub fn repository(mut self, repository: &str) -> Self {
        if !repository.is_empty() {
            self.config.repository = Some(repository.to_string());
        }
        self
    }

    /// Set the Git reference to resolve.
    ///
    /// The value may be a branch, tag, commit hash, or `HEAD`. It is resolved
    /// once when the operator first accesses the repository and remains fixed for
    /// the lifetime of that operator. The default is `HEAD`.
    pub fn reference(mut self, reference: &str) -> Self {
        if !reference.is_empty() {
            self.config.reference = Some(reference.to_string());
        }
        self
    }

    /// Set the root path within the repository.
    ///
    /// All operations are relative to this path. The default is `/`.
    pub fn root(mut self, root: &str) -> Self {
        self.config.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };
        self
    }

    /// Set the username for the Git fetch and same-origin LFS requests.
    pub fn username(mut self, username: &str) -> Self {
        if !username.is_empty() {
            self.config.username = Some(username.to_string());
        }
        self
    }

    /// Set the password or personal access token for the Git fetch and
    /// same-origin LFS requests.
    pub fn password(mut self, password: &str) -> Self {
        if !password.is_empty() {
            self.config.password = Some(password.to_string());
        }
        self
    }

    /// Set whether Git LFS pointer files are resolved.
    ///
    /// When enabled, the service streams the referenced LFS object. When
    /// disabled, `read` returns the pointer file itself. The default is `true`.
    pub fn resolve_lfs(mut self, resolve_lfs: bool) -> Self {
        self.config.resolve_lfs = Some(resolve_lfs);
        self
    }
}

impl Builder for GitBuilder {
    type Config = GitConfig;

    fn build(self) -> Result<impl Service> {
        debug!("git backend build started");

        let repository = self.config.repository.clone().ok_or_else(|| {
            Error::new(ErrorKind::ConfigInvalid, "repository is required")
                .with_context("service", GIT_SCHEME)
        })?;
        let reference = self
            .config
            .reference
            .clone()
            .unwrap_or_else(|| "HEAD".to_string());
        let root = normalize_root(&self.config.root.clone().unwrap_or_default());
        let resolve_lfs = self.config.resolve_lfs.unwrap_or(true);

        let core = Arc::new(GitCore::new(
            repository,
            reference,
            root.clone(),
            self.config.username.clone(),
            self.config.password.clone(),
            resolve_lfs,
        ));

        let info = ServiceInfo::new(GIT_SCHEME, &root, "");
        let capability = Capability {
            stat: true,
            read: true,
            list: true,
            shared: true,
            ..Default::default()
        };

        debug!("git backend build finished");
        Ok(GitBackend {
            core,
            info,
            capability,
        })
    }
}

/// Git service backend.
#[derive(Debug, Clone)]
pub struct GitBackend {
    core: Arc<GitCore>,
    info: ServiceInfo,
    capability: Capability,
}

impl Service for GitBackend {
    type Reader = oio::StreamReader<GitReader>;
    type Writer = ();
    type Lister = GitLister;
    type Deleter = ();
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        self.info.clone()
    }

    fn capability(&self) -> Capability {
        self.capability
    }

    async fn create_dir(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(unsupported("create_dir"))
    }

    async fn stat(&self, _ctx: &OperationContext, path: &str, _args: OpStat) -> Result<RpStat> {
        let core = self.core.clone();
        let path = path.to_string();
        let (size, is_dir, last_modified) =
            tokio::task::spawn_blocking(move || core.stat_file_blocking(&path))
                .await
                .map_err(join_error)??;

        let mut builder = if is_dir {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::file(size)
        };
        if let Ok(timestamp) = Timestamp::from_second(last_modified) {
            builder.last_modified(timestamp);
        }
        Ok(RpStat::new(builder.build()))
    }

    fn read(&self, ctx: &OperationContext, path: &str, _args: OpRead) -> Result<Self::Reader> {
        Ok(oio::StreamReader::new(GitReader::new(
            self.core.clone(),
            ctx.clone(),
            path.to_string(),
        )))
    }

    fn write(&self, _ctx: &OperationContext, _path: &str, _args: OpWrite) -> Result<()> {
        Err(unsupported("write"))
    }

    fn delete(&self, _ctx: &OperationContext) -> Result<()> {
        Err(unsupported("delete"))
    }

    fn list(&self, _ctx: &OperationContext, path: &str, _args: OpList) -> Result<Self::Lister> {
        Ok(GitLister::new(self.core.clone(), path.to_string()))
    }

    fn copy(&self, _ctx: &OperationContext, _from: &str, _to: &str, _args: OpCopy) -> Result<()> {
        Err(unsupported("copy"))
    }

    async fn rename(
        &self,
        _ctx: &OperationContext,
        _from: &str,
        _to: &str,
        _args: OpRename,
    ) -> Result<RpRename> {
        Err(unsupported("rename"))
    }

    async fn presign(
        &self,
        _ctx: &OperationContext,
        _path: &str,
        _args: OpPresign,
    ) -> Result<RpPresign> {
        Err(unsupported("presign"))
    }
}

fn unsupported(operation: &'static str) -> Error {
    Error::new(
        ErrorKind::Unsupported,
        format!("{operation} is not supported by the git service"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use opendal_core::raw::Service as _;
    use opendal_core::raw::oio::List as _;
    use opendal_core::raw::oio::Read as _;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git must be installed for the git service tests");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn init_repo() -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().to_path_buf();
        git(&dir, &["init", "-q"]);
        git(&dir, &["config", "user.email", "opendal@example.com"]);
        git(&dir, &["config", "user.name", "OpenDAL"]);
        git(&dir, &["config", "commit.gpgsign", "false"]);
        (temp, dir)
    }

    fn commit_all(dir: &Path, message: &str) {
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", message]);
    }

    fn file_url(dir: &Path) -> String {
        url::Url::from_file_path(dir).unwrap().to_string()
    }

    #[tokio::test]
    async fn local_snapshot_root_and_plain_blob() {
        let (_temp, dir) = init_repo();
        std::fs::create_dir_all(dir.join("root")).unwrap();
        std::fs::write(dir.join("root/plain.txt"), b"hello").unwrap();
        std::fs::write(dir.join("state.txt"), b"one").unwrap();
        commit_all(&dir, "initial");
        let first = git(&dir, &["rev-parse", "HEAD"]);

        std::fs::write(dir.join("state.txt"), b"two").unwrap();
        commit_all(&dir, "second");
        let second = git(&dir, &["rev-parse", "HEAD"]);

        let ctx = OperationContext::new();
        let first_service = GitBuilder::default()
            .repository(&file_url(&dir))
            .reference(&first)
            .resolve_lfs(false)
            .build()
            .unwrap();
        let reader = first_service
            .read(&ctx, "state.txt", OpRead::default())
            .unwrap();
        let (_, data) = reader.read(BytesRange::default()).await.unwrap();
        assert_eq!(data.to_bytes(), Bytes::from_static(b"one"));

        let head_service = GitBuilder::default()
            .repository(&file_url(&dir))
            .reference(&second)
            .root("/root/")
            .resolve_lfs(false)
            .build()
            .unwrap();
        let stat = head_service
            .stat(&ctx, "/", OpStat::default())
            .await
            .unwrap();
        assert!(stat.metadata().mode().is_dir());

        let mut lister = head_service.list(&ctx, "/", OpList::default()).unwrap();
        let entry = lister.next().await.unwrap().unwrap();
        assert_eq!(entry.path(), "plain.txt");
        assert!(lister.next().await.unwrap().is_none());

        let reader = head_service
            .read(&ctx, "plain.txt", OpRead::default())
            .unwrap();
        let (_, data) = reader.read(BytesRange::new(0, Some(2))).await.unwrap();
        assert_eq!(data.to_bytes(), Bytes::from_static(b"he"));
    }

    #[tokio::test]
    async fn resolve_lfs_false_returns_pointer() {
        let (_temp, dir) = init_repo();
        let pointer = format!(
            "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize 4\n",
            "0".repeat(64)
        );
        std::fs::write(dir.join("model.bin"), &pointer).unwrap();
        commit_all(&dir, "pointer");
        let head = git(&dir, &["rev-parse", "HEAD"]);

        let service = GitBuilder::default()
            .repository(&file_url(&dir))
            .reference(&head)
            .resolve_lfs(false)
            .build()
            .unwrap();
        let ctx = OperationContext::new();
        let stat = service
            .stat(&ctx, "model.bin", OpStat::default())
            .await
            .unwrap();
        assert_eq!(stat.metadata().content_length(), pointer.len() as u64);

        let reader = service.read(&ctx, "model.bin", OpRead::default()).unwrap();
        let (_, data) = reader.read(BytesRange::default()).await.unwrap();
        assert_eq!(data.to_bytes(), Bytes::from(pointer.into_bytes()));
    }

    #[tokio::test]
    async fn submodule_entry_is_unsupported() {
        let (_temp, dir) = init_repo();
        std::fs::write(dir.join("plain.txt"), b"hello").unwrap();
        commit_all(&dir, "initial");
        let head = git(&dir, &["rev-parse", "HEAD"]);
        let cacheinfo = format!("160000,{head},submodule");
        git(&dir, &["update-index", "--add", "--cacheinfo", &cacheinfo]);
        git(&dir, &["commit", "-q", "-m", "submodule"]);
        let head = git(&dir, &["rev-parse", "HEAD"]);

        let service = GitBuilder::default()
            .repository(&file_url(&dir))
            .reference(&head)
            .resolve_lfs(false)
            .build()
            .unwrap();
        let ctx = OperationContext::new();
        let err = service
            .stat(&ctx, "submodule", OpStat::default())
            .await
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Unsupported);
    }
}
