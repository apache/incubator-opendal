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
use std::path::Path;
use std::path::PathBuf;

use opendal_core::raw::*;
use opendal_core::*;

#[derive(Debug)]
pub struct FsCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub root: PathBuf,
    pub atomic_write_dir: Option<PathBuf>,
    pub buf_pool: oio::PooledBuf,
}

impl FsCore {
    /// Join a caller-supplied key onto a base directory while keeping the result
    /// confined to that base.
    ///
    /// `normalize_path` (opendal-core) strips leading `/` and empty segments but
    /// intentionally does NOT resolve `.`/`..`, and `PathBuf::join` is purely
    /// lexical, so a key such as `../../etc/passwd` would otherwise escape the
    /// configured `root` at syscall time. The fs backend documents that "all
    /// operations will happen under this root", so we reject any key whose
    /// components include a `..` (parent-dir) traversal, a root directory, or a
    /// path prefix: `PathBuf::join` discards the base when the key is absolute,
    /// which covers `/etc/passwd` plus the Windows `C:\`, `\\?\` and UNC forms
    /// that `normalize_path` leaves untouched.
    pub fn confined_join(base: &Path, path: &str) -> Result<PathBuf> {
        use std::path::Component;
        let trimmed = path.trim_end_matches('/');
        if Path::new(trimmed).components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(
                Error::new(ErrorKind::NotFound, "path escapes the configured root")
                    .with_context("path", path),
            );
        }
        Ok(base.join(trimmed))
    }

    /// Join a caller-supplied key onto `self.root`, keeping the result confined.
    #[inline]
    pub fn root_join(&self, path: &str) -> Result<PathBuf> {
        Self::confined_join(&self.root, path)
    }

    // Build write path and ensure the parent dirs created
    pub async fn ensure_write_abs_path(&self, parent: &Path, path: &str) -> Result<PathBuf> {
        let p = Self::confined_join(parent, path)?;

        // Create dir before write path.
        //
        // TODO(xuanwo): There are many works to do here:
        //   - Is it safe to create dir concurrently?
        //   - Do we need to extract this logic as new util functions?
        //   - Is it better to check the parent dir exists before call mkdir?
        let parent = PathBuf::from(&p)
            .parent()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Unexpected,
                    "path should have parent but not, it must be malformed",
                )
                .with_context("input", p.to_string_lossy())
            })?
            .to_path_buf();

        tokio::fs::create_dir_all(&parent)
            .await
            .map_err(new_std_io_error)?;

        Ok(p)
    }

    pub async fn fs_create_dir(&self, path: &str) -> Result<()> {
        let p = self.root_join(path)?;
        tokio::fs::create_dir_all(&p)
            .await
            .map_err(new_std_io_error)?;
        Ok(())
    }

    pub async fn fs_stat(&self, path: &str) -> Result<Metadata> {
        let p = self.root_join(path)?;
        let meta = tokio::fs::metadata(&p).await.map_err(new_std_io_error)?;
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
        m.last_modified(Timestamp::try_from(
            meta.modified().map_err(new_std_io_error)?,
        )?);

        // Read user metadata from xattr on Unix systems
        #[cfg(unix)]
        {
            let user_metadata = Self::get_user_metadata(&p)?;
            if !user_metadata.is_empty() {
                m.user_metadata(user_metadata);
            }
        }

        Ok(m.build())
    }

    pub async fn fs_open(&self, path: &str) -> Result<File> {
        let p = self.root_join(path)?;

        let f = tokio::fs::OpenOptions::new()
            .read(true)
            .open(&p)
            .await
            .map_err(new_std_io_error)?
            .into_std()
            .await;

        Ok(f)
    }

    pub async fn fs_write(&self, path: &PathBuf, op: &OpWrite) -> Result<tokio::fs::File> {
        let mut open_options = tokio::fs::OpenOptions::new();
        if op.if_not_exists() {
            open_options.create_new(true);
        } else {
            open_options.create(true);
        }

        open_options.write(true);

        if op.append() {
            open_options.append(true);
        } else {
            open_options.truncate(true);
        }

        let f = open_options
            .open(path)
            .await
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("OpenForWrite")), err))?;

        Ok(f)
    }

    /// This function is used to build a tempfile for writing.
    ///
    /// We don't care about the OpWrite since every check should be performed on target path directly.
    pub async fn fs_tempfile_write(
        &self,
        path: &str,
    ) -> Result<(tokio::fs::File, Option<PathBuf>)> {
        let Some(atomic_write_dir) = self.atomic_write_dir.as_ref() else {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "fs didn't configure atomic_write_dir, but we're still entering the tempfile logic. This might be a bug.",
            ));
        };

        let tmp_path = self
            .ensure_write_abs_path(atomic_write_dir, &build_tmp_path_of(path))
            .await?;

        let mut open_options = tokio::fs::OpenOptions::new();

        // tempfile should always be new file.
        open_options.create_new(true);
        open_options.write(true);
        open_options.truncate(true);

        let f = open_options.open(&tmp_path).await.map_err(|err| {
            parse_error(
                ErrorContext::new(ServiceOperation("OpenTemporaryFile")),
                err,
            )
        })?;

        Ok((f, Some(tmp_path)))
    }

    pub async fn fs_list(&self, path: &str) -> Result<Option<tokio::fs::ReadDir>> {
        let p = self.root_join(path)?;

        match tokio::fs::read_dir(&p).await {
            Ok(rd) => Ok(Some(rd)),
            Err(e) => {
                match e.kind() {
                    // Return empty list if the directory not found
                    std::io::ErrorKind::NotFound => Ok(None),
                    // If the path is not a directory, return an empty list
                    //
                    // The path could be a file or a symbolic link in this case.
                    // Returning a NotADirectory error to the user isn't helpful; instead,
                    // providing an empty directory is a more user-friendly. In fact, the dir
                    // `path/` does not exist.
                    std::io::ErrorKind::NotADirectory => Ok(None),
                    _ => Err(new_std_io_error(e)),
                }
            }
        }
    }

    pub async fn fs_copy(&self, from: &str, to: &str) -> Result<u64> {
        let from = self.root_join(from)?;
        // try to get the metadata of the source file to ensure it exists
        tokio::fs::metadata(&from).await.map_err(new_std_io_error)?;

        let to = self
            .ensure_write_abs_path(&self.root, to.trim_end_matches('/'))
            .await?;

        let size = tokio::fs::copy(&from, &to)
            .await
            .map_err(new_std_io_error)?;

        // only *nix supports `write_with_user_metadata`
        #[cfg(unix)]
        {
            if let Ok(user_meta) = Self::get_user_metadata(&from)
                && !user_meta.is_empty()
            {
                Self::set_user_metadata(&to, &user_meta)?;
            }
        }

        Ok(size)
    }

    pub async fn fs_rename(&self, from: &str, to: &str) -> Result<()> {
        let from = self.root_join(from)?;
        tokio::fs::metadata(&from).await.map_err(new_std_io_error)?;

        let to = self
            .ensure_write_abs_path(&self.root, to.trim_end_matches('/'))
            .await?;

        tokio::fs::rename(from, to)
            .await
            .map_err(new_std_io_error)?;
        Ok(())
    }

    /// Get user metadata from extended attributes on Unix systems.
    ///
    /// Reads xattr in the "user." namespace and strips the prefix, allowing
    /// interoperability with other applications that set user xattr.
    #[cfg(unix)]
    pub fn get_user_metadata(path: &Path) -> Result<HashMap<String, String>> {
        let mut user_metadata = HashMap::new();

        let attrs = match xattr::list(path) {
            Ok(attrs) => attrs,
            Err(e) => {
                // xattr may not be supported on some filesystems, ignore the error
                if e.kind() == std::io::ErrorKind::Unsupported {
                    return Ok(user_metadata);
                }
                // Also ignore "operation not supported" errors (ENOTSUP/EOPNOTSUPP)
                // which may occur on filesystems that don't support xattr
                if let Some(os_error) = e.raw_os_error() {
                    // ENOTSUP (95) and EOPNOTSUPP (95) on Linux
                    if os_error == 95 {
                        return Ok(user_metadata);
                    }
                }
                return Err(new_std_io_error(e));
            }
        };

        for attr in attrs {
            let attr_name = attr.to_string_lossy();
            // Only read xattr in the "user." namespace and strip the prefix
            if let Some(key) = attr_name.strip_prefix(XATTR_USER_PREFIX)
                && let Ok(Some(value)) = xattr::get(path, &attr)
                && let Ok(v) = String::from_utf8(value)
            {
                user_metadata.insert(key.to_string(), v);
            }
        }

        Ok(user_metadata)
    }

    /// Set user metadata as extended attributes on Unix systems.
    ///
    /// Keys are automatically prefixed with "user." to comply with Linux xattr
    /// naming requirements. This allows interoperability with other applications.
    #[cfg(unix)]
    pub fn set_user_metadata(path: &Path, user_metadata: &HashMap<String, String>) -> Result<()> {
        for (key, value) in user_metadata {
            let attr_name = format!("{XATTR_USER_PREFIX}{key}");
            xattr::set(path, &attr_name, value.as_bytes()).map_err(new_std_io_error)?;
        }
        Ok(())
    }
}

/// Prefix for user xattr namespace on Linux.
/// Using "user." as the standard namespace for user-defined attributes.
#[cfg(unix)]
const XATTR_USER_PREFIX: &str = "user.";

/// Context needed to classify an error from this service.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrorContext {
    service_operation: ServiceOperation,
}

impl ErrorContext {
    pub(crate) const fn new(service_operation: ServiceOperation) -> Self {
        Self { service_operation }
    }
}

/// Parse an error using its service operation context.
pub(crate) fn parse_error(ctx: ErrorContext, err: std::io::Error) -> Error {
    let err = match err.kind() {
        std::io::ErrorKind::AlreadyExists => Error::new(
            ErrorKind::ConditionNotMatch,
            "The file already exists in the filesystem",
        )
        .set_source(err),
        _ => new_std_io_error(err),
    };

    err.with_context("service_operation", ctx.service_operation.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn test_fs_backend_copy_preserves_user_metadata() {
        use opendal_core::Operator;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path();

        let src = "src_meta.txt";
        let dst = "dst_meta.txt";

        let src_path = root.join(src);
        let dst_path = root.join(dst);

        std::fs::File::create(&src_path).unwrap();

        let mut meta = HashMap::new();
        meta.insert("key".to_string(), "preserved123".to_string());
        FsCore::set_user_metadata(&src_path, &meta).unwrap();

        let op = Operator::new(crate::Fs::default().root(root.to_str().unwrap())).unwrap();
        op.copy(src, dst).await.unwrap();

        let got = FsCore::get_user_metadata(&dst_path).unwrap();
        assert_eq!(got.get("key").map(String::as_str), Some("preserved123"));
    }

    #[test]
    fn test_confined_join_rejects_escaping_keys() {
        let base = Path::new("/data/root");
        for key in ["../etc/passwd", "a/../../b", "/etc/passwd", "//etc/passwd"] {
            let err = FsCore::confined_join(base, key).unwrap_err();
            assert_eq!(
                err.kind(),
                ErrorKind::NotFound,
                "key should be rejected: {key}"
            );
        }
    }

    #[test]
    fn test_confined_join_allows_normal_keys() {
        let base = Path::new("/data/root");
        assert_eq!(
            FsCore::confined_join(base, "a/b.txt").unwrap(),
            PathBuf::from("/data/root/a/b.txt")
        );
        assert_eq!(
            FsCore::confined_join(base, "a/b/").unwrap(),
            PathBuf::from("/data/root/a/b")
        );
        // The root path `/` trims to empty and resolves to the base itself.
        assert_eq!(
            FsCore::confined_join(base, "/").unwrap(),
            PathBuf::from("/data/root")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_delete_iter_normalizes_absolute_path_under_root() {
        use opendal_core::Operator;

        let root_dir = tempfile::TempDir::new().unwrap();
        let outside_dir = tempfile::TempDir::new().unwrap();

        let outside = outside_dir.path().join("outside.txt");
        std::fs::write(&outside, b"content").unwrap();

        let inside = root_dir.path().join(outside.strip_prefix("/").unwrap());
        std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
        std::fs::write(&inside, b"content").unwrap();

        let op =
            Operator::new(crate::Fs::default().root(root_dir.path().to_str().unwrap())).unwrap();

        op.delete_iter([outside.to_str().unwrap().to_string()])
            .await
            .unwrap();
        assert!(!inside.exists());
        assert_eq!(std::fs::read(&outside).unwrap(), b"content");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_list_includes_dangling_symlink() {
        use opendal_core::Operator;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path();

        std::fs::write(root.join("real"), b"data").unwrap();
        std::os::unix::fs::symlink("/nonexistent-target", root.join("linky")).unwrap();
        std::os::unix::fs::symlink(root.join("real"), root.join("valid-link")).unwrap();

        let op = Operator::new(crate::Fs::default().root(root.to_str().unwrap())).unwrap();
        let entries = op.list("/").await.unwrap();
        let modes: std::collections::HashMap<String, EntryMode> = entries
            .iter()
            .map(|e| (e.path().to_string(), e.metadata().mode()))
            .collect();

        assert_eq!(
            modes.get("linky"),
            Some(&EntryMode::Unknown),
            "dangling symlink must be listed as Unknown: {modes:?}"
        );
        assert_eq!(modes.get("real"), Some(&EntryMode::FILE));
    }

    #[tokio::test]
    async fn test_delete_iter_rejects_parent_path_outside_root() {
        use opendal_core::Operator;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path().join("root");
        std::fs::create_dir(&root).unwrap();

        let outside = temp_dir.path().join("outside.txt");
        std::fs::write(&outside, b"content").unwrap();

        let op = Operator::new(crate::Fs::default().root(root.to_str().unwrap())).unwrap();
        let err = op.delete_iter(["../outside.txt"]).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert_eq!(std::fs::read(&outside).unwrap(), b"content");
    }
}
