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
use smb::CreateOptions;
use smb::File;
use smb::FileAccessMask;
use smb::FileCreateArgs;
use smb::Resource;

use super::core::RuntimeResource;
use super::core::SmbCore;
use super::error::parse_smb_error;

pub(super) struct SmbReader {
    core: Arc<SmbCore>,
    path: String,
}

impl SmbReader {
    pub fn new(core: Arc<SmbCore>, path: &str) -> Self {
        Self {
            core,
            path: path.to_string(),
        }
    }
}

pub(super) struct SmbReadHandle {
    file: RuntimeResource<File>,
    max_read_size: usize,
}

impl oio::PositionRead for SmbReader {
    type Handle = SmbReadHandle;

    async fn open(&self) -> Result<Self::Handle> {
        let target = self.core.path(&self.path)?;
        let client = self.core.client().await?;
        let connection = client
            .get_connection(self.core.share.server())
            .await
            .map_err(parse_smb_error)?;
        let max_read_size = (connection
            .conn_info()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "SMB connection was not negotiated"))?
            .negotiation
            .max_read_size as usize)
            .min(64 * 1024);
        let mut args =
            FileCreateArgs::make_open_existing(FileAccessMask::new().with_generic_read(true));
        args.options = CreateOptions::new().with_non_directory_file(true);
        let resource = client
            .create_file(&target, &args)
            .await
            .map_err(parse_smb_error)?;
        match resource {
            Resource::File(file) => Ok(SmbReadHandle {
                file: RuntimeResource::new(file),
                max_read_size,
            }),
            Resource::Directory(_) => Err(Error::new(
                ErrorKind::IsADirectory,
                "cannot read a directory",
            )),
            Resource::Pipe(_) => Err(Error::new(
                ErrorKind::Unsupported,
                "SMB pipes are not supported",
            )),
        }
    }

    async fn read_at(handle: &Self::Handle, offset: u64, size: usize) -> Result<Buffer> {
        let mut buffer = vec![0; size.min(handle.max_read_size)];
        let read = handle
            .file
            .read_block(&mut buffer, offset, None, false)
            .await
            .map_err(new_std_io_error)?;
        buffer.truncate(read);
        Ok(Buffer::from(buffer))
    }
}
