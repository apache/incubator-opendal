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

use futures::StreamExt;
use opendal_core::raw::*;
use opendal_core::*;
use smb::CreateOptions;
use smb::Directory;
use smb::FileAccessMask;
use smb::FileCreateArgs;
use smb::FileDirectoryInformation;
use smb::Resource;
use tokio::sync::mpsc;

use super::core::RuntimeResource;
use super::core::SmbCore;
use super::core::to_metadata;
use super::error::parse_smb_error;

pub(super) struct SmbLister {
    core: Arc<SmbCore>,
    path: String,
    receiver: Option<mpsc::Receiver<Result<oio::Entry>>>,
}

impl SmbLister {
    pub fn new(core: Arc<SmbCore>, path: &str) -> Self {
        Self {
            core,
            path: path.to_string(),
            receiver: None,
        }
    }

    async fn query(
        core: Arc<SmbCore>,
        path: String,
        sender: &mpsc::Sender<Result<oio::Entry>>,
    ) -> Result<()> {
        let target = core.path(&path)?;
        let client = core.client().await?;
        let mut args =
            FileCreateArgs::make_open_existing(FileAccessMask::new().with_generic_read(true));
        args.options = CreateOptions::new().with_directory_file(true);
        let resource = match client.create_file(&target, &args).await {
            Ok(resource) => resource,
            Err(error) => {
                let error = parse_smb_error(error);
                return if error.kind() == ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(error)
                };
            }
        };
        let directory = match resource {
            Resource::Directory(dir) => RuntimeResource::new(Arc::new(dir)),
            _ => {
                return Err(Error::new(ErrorKind::NotADirectory, "cannot list a file"));
            }
        };
        let mut entries = match Directory::query::<FileDirectoryInformation>(&directory, "*").await
        {
            Ok(entries) => entries,
            Err(error) => {
                directory.close().await.map_err(parse_smb_error)?;
                return Err(parse_smb_error(error));
            }
        };
        let result = async {
            loop {
                let entry = tokio::select! {
                    biased;
                    _ = sender.closed() => return Ok(()),
                    entry = entries.next() => entry,
                };
                let Some(entry) = entry else {
                    return Ok(());
                };
                let entry = entry.map_err(parse_smb_error)?;
                let name = entry.file_name.to_string();
                if name == ".." {
                    continue;
                }
                let prefix = if path == "/" { "" } else { path.as_str() };
                let mut entry_path = if name == "." {
                    if prefix.is_empty() {
                        "/".to_string()
                    } else {
                        prefix.to_string()
                    }
                } else {
                    format!("{prefix}{name}")
                };
                if entry.file_attributes.directory() && !entry_path.ends_with('/') {
                    entry_path.push('/');
                }
                let metadata = to_metadata(
                    entry.file_attributes,
                    entry.end_of_file,
                    entry.last_write_time,
                )?;
                if sender
                    .send(Ok(oio::Entry::new(&entry_path, metadata)))
                    .await
                    .is_err()
                {
                    return Ok(());
                };
            }
        }
        .await;

        let close_result = directory.close().await.map_err(parse_smb_error);
        // smb-rs keeps the directory alive in its query task. Drain the buffered
        // page after closing so the task observes the closed handle and exits,
        // including when the caller drops the lister before reaching EOF.
        while let Some(entry) = entries.next().await {
            if entry.is_err() {
                break;
            }
        }
        result.and(close_result)
    }
}

impl oio::List for SmbLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        if self.receiver.is_none() {
            let core = self.core.clone();
            let path = self.path.clone();
            let (sender, receiver) = mpsc::channel(16);
            self.receiver = Some(receiver);
            tokio::spawn(async move {
                if let Err(error) = Self::query(core, path, &sender).await {
                    let _ = sender.send(Err(error)).await;
                }
            });
        }
        self.receiver
            .as_mut()
            .expect("lister is initialized")
            .recv()
            .await
            .transpose()
    }
}
