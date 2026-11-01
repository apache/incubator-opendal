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

use std::fmt;
use std::ops::Deref;

use opendal_core::raw::*;
use opendal_core::*;
use smb::Client;
use smb::ClientConfig;
use smb::CreateDisposition;
use smb::CreateOptions;
use smb::FileAccessMask;
use smb::FileAttributes;
use smb::FileCreateArgs;
use smb::Resource;
use smb::UncPath;
use smb::binrw_util::prelude::FileTime;
use tokio::runtime::Handle;
use tokio::sync::OnceCell;

use super::error::parse_smb_error;

pub(super) struct SmbCore {
    pub info: ServiceInfo,
    pub root: String,
    pub share: UncPath,
    pub username: String,
    pub password: String,
    pub client_config: ClientConfig,
    pub client: OnceCell<RuntimeResource<Client>>,
}

// smb-rs destructors spawn asynchronous cleanup. Retain their runtime context
// so dropping an operator or handle outside a runtime does not panic.
pub(super) struct RuntimeResource<T> {
    inner: Option<T>,
    runtime: Handle,
}

impl<T> RuntimeResource<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Some(inner),
            runtime: Handle::current(),
        }
    }
}

impl<T> Deref for RuntimeResource<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner.as_ref().expect("resource has not been dropped")
    }
}

impl<T> Drop for RuntimeResource<T> {
    fn drop(&mut self) {
        let _guard = self.runtime.enter();
        drop(self.inner.take());
    }
}

impl fmt::Debug for SmbCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmbCore")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl SmbCore {
    pub async fn client(&self) -> Result<&Client> {
        self.client
            .get_or_try_init(|| async {
                let client = RuntimeResource::new(Client::new(self.client_config.clone()));
                client
                    .share_connect(&self.share, &self.username, self.password.clone())
                    .await
                    .map_err(parse_smb_error)?;
                Ok(client)
            })
            .await
            .map(|client| &**client)
    }

    pub fn path(&self, path: &str) -> Result<UncPath> {
        let path = build_abs_path(&self.root, path);
        // SMB treats backslashes as separators; accepting them would bypass the
        // OpenDAL root boundary even though OpenDAL normalizes forward slashes.
        if path.contains(['\\', '\0']) || path.split('/').any(|part| matches!(part, "." | "..")) {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "SMB paths must not contain backslashes, NUL, or dot components",
            ));
        }
        Ok(self.share.clone().with_path(path.trim_matches('/')))
    }

    pub async fn create_dir(&self, path: &str) -> Result<()> {
        let target = self.path(path)?;
        let client = self.client().await?;
        let args = FileCreateArgs {
            disposition: CreateDisposition::OpenIf,
            attributes: FileAttributes::new().with_directory(true),
            options: CreateOptions::new().with_directory_file(true),
            desired_access: FileAccessMask::new().with_file_read_attributes(true),
        };
        let mut current = String::new();
        for part in target
            .path()
            .unwrap_or_default()
            .split('\\')
            .filter(|p| !p.is_empty())
        {
            if !current.is_empty() {
                current.push('\\');
            }
            current.push_str(part);
            let resource = client
                .create_file(&self.share.clone().with_path(&current), &args)
                .await
                .map_err(parse_smb_error)?;
            // On a disk share, FILE_DIRECTORY_FILE guarantees a directory on success.
            // Close the handle without relying on its response attribute bit.
            match resource {
                Resource::Directory(dir) => RuntimeResource::new(dir)
                    .close()
                    .await
                    .map_err(parse_smb_error)?,
                Resource::File(file) => RuntimeResource::new(file)
                    .close()
                    .await
                    .map_err(parse_smb_error)?,
                _ => {
                    return Err(Error::new(
                        ErrorKind::NotADirectory,
                        "parent is not a directory",
                    ));
                }
            }
        }
        Ok(())
    }
}

pub(super) fn to_metadata(
    attributes: FileAttributes,
    size: u64,
    modified: FileTime,
) -> Result<Metadata> {
    let mut metadata = if attributes.directory() {
        MetadataBuilder::dir()
    } else {
        MetadataBuilder::file(size)
    };
    if !modified.is_zero() {
        let duration = modified.since_epoch();
        // FILETIME counts 100 ns intervals from 1601; OpenDAL uses the Unix epoch.
        metadata.last_modified(Timestamp::new(
            duration.as_secs() as i64 - 11_644_473_600,
            duration.subsec_nanos() as i32,
        )?);
    }
    Ok(metadata.build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Smb;

    #[test]
    fn paths_stay_relative_to_share_and_root() {
        let core = SmbCore {
            info: ServiceInfo::new("smb", "/root/", "data"),
            root: "/root/".to_string(),
            share: UncPath::new("host").unwrap().with_share("data").unwrap(),
            username: "user".to_string(),
            password: "password".to_string(),
            client_config: ClientConfig::default(),
            client: OnceCell::new(),
        };
        assert_eq!(
            core.path("nested/file").unwrap().path(),
            Some(r"root\nested\file")
        );
        assert_eq!(core.path("/").unwrap().path(), Some("root"));
        for path in [
            "../file",
            r"..\file",
            "nested/../../file",
            "nested/./file",
            "file\0",
        ] {
            assert_eq!(
                core.path(path).unwrap_err().kind(),
                ErrorKind::ConfigInvalid
            );
        }
        let debug = format!("{core:?}");
        assert!(!debug.contains("password"));
        assert!(!format!("{:?}", Smb::default().password("secret")).contains("secret"));
    }

    #[test]
    fn metadata_converts_filetime_without_losing_subseconds() {
        let metadata = to_metadata(
            FileAttributes::new(),
            42,
            FileTime::from(116_444_736_001_234_567),
        )
        .unwrap();
        assert_eq!(metadata.content_length(), 42);
        assert_eq!(
            metadata.last_modified(),
            Some(Timestamp::new(0, 123_456_700).unwrap())
        );
        assert!(
            to_metadata(
                FileAttributes::new().with_directory(true),
                0,
                FileTime::ZERO
            )
            .unwrap()
            .is_dir()
        );
    }
}
