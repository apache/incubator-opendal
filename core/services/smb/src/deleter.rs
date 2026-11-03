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

use opendal_core::raw::*;
use opendal_core::*;
use smb::FileAccessMask;
use smb::FileCreateArgs;
use smb::FileDispositionInformation;
use smb::Resource;

use super::core::RuntimeResource;
use super::core::SmbCore;
use super::error::parse_smb_error;

pub(super) struct SmbDeleter {
    core: Arc<SmbCore>,
}

impl SmbDeleter {
    pub fn new(core: Arc<SmbCore>) -> Self {
        Self { core }
    }
}

impl oio::OneShotDelete for SmbDeleter {
    async fn delete_once(&self, path: String, _: OpDelete) -> Result<()> {
        let target = self.core.path(&path)?;
        let client = self.core.client().await?;
        let args = FileCreateArgs::make_open_existing(FileAccessMask::new().with_delete(true));
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
        let result = match resource {
            Resource::File(file) => {
                let file = RuntimeResource::new(file);
                let result = file.set_info(FileDispositionInformation::default()).await;
                file.close().await.map_err(parse_smb_error)?;
                result
            }
            Resource::Directory(dir) => {
                let dir = RuntimeResource::new(dir);
                let result = dir.set_info(FileDispositionInformation::default()).await;
                dir.close().await.map_err(parse_smb_error)?;
                result
            }
            Resource::Pipe(_) => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "SMB pipes are not supported",
                ));
            }
        };
        result.map_err(parse_smb_error)
    }
}
