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
use smb::CreateDisposition;
use smb::CreateOptions;
use smb::File;
use smb::FileAccessMask;
use smb::FileAllInformation;
use smb::FileAttributes;
use smb::FileCreateArgs;
use smb::Resource;
use smb::UncPath;

use super::core::RuntimeResource;
use super::core::SmbCore;
use super::core::to_metadata;
use super::error::parse_smb_error;

pub(super) struct SmbCopier;

impl SmbCopier {
    pub async fn copy(
        core: Arc<SmbCore>,
        source: UncPath,
        target: UncPath,
        parent: Option<String>,
        op: OpCopy,
    ) -> Result<Metadata> {
        let client = core.client().await?;
        let mut source_args =
            FileCreateArgs::make_open_existing(FileAccessMask::new().with_generic_read(true));
        source_args.options = CreateOptions::new().with_non_directory_file(true);
        let source_file = match client
            .create_file(&source, &source_args)
            .await
            .map_err(parse_smb_error)?
        {
            Resource::File(file) => RuntimeResource::new(file),
            Resource::Directory(dir) => {
                RuntimeResource::new(dir)
                    .close()
                    .await
                    .map_err(parse_smb_error)?;
                return Err(Error::new(
                    ErrorKind::IsADirectory,
                    "cannot copy a directory",
                ));
            }
            Resource::Pipe(_) => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "SMB pipes are not supported",
                ));
            }
        };

        let result = async {
            if let Some(parent) = parent {
                core.create_dir(&parent).await?;
            }
            let target_args = FileCreateArgs {
                disposition: if op.if_not_exists() {
                    CreateDisposition::Create
                } else {
                    CreateDisposition::OverwriteIf
                },
                attributes: FileAttributes::new(),
                options: CreateOptions::new().with_non_directory_file(true),
                desired_access: FileAccessMask::new()
                    .with_generic_write(true)
                    .with_file_read_attributes(true),
            };
            let target_file =
                match client
                    .create_file(&target, &target_args)
                    .await
                    .map_err(|error| {
                        let error = parse_smb_error(error);
                        if op.if_not_exists() && error.kind() == ErrorKind::AlreadyExists {
                            Error::new(ErrorKind::ConditionNotMatch, "file already exists")
                                .set_source(error)
                        } else {
                            error
                        }
                    })? {
                    Resource::File(file) => RuntimeResource::new(file),
                    Resource::Directory(dir) => {
                        RuntimeResource::new(dir)
                            .close()
                            .await
                            .map_err(parse_smb_error)?;
                        return Err(Error::new(
                            ErrorKind::IsADirectory,
                            "cannot copy to a directory",
                        ));
                    }
                    Resource::Pipe(_) => {
                        return Err(Error::new(
                            ErrorKind::Unsupported,
                            "SMB pipes are not supported",
                        ));
                    }
                };

            let result = Self::transfer(&core, &source_file, &target_file).await;
            let closed = target_file.close().await.map_err(parse_smb_error);
            result.and_then(|metadata| closed.map(|()| metadata))
        }
        .await;
        let closed = source_file.close().await.map_err(parse_smb_error);
        result.and_then(|metadata| closed.map(|()| metadata))
    }

    async fn transfer(core: &SmbCore, source: &File, target: &File) -> Result<Metadata> {
        let client = core.client().await?;
        let connection = client
            .get_connection(core.share.server())
            .await
            .map_err(parse_smb_error)?;
        let negotiation = &connection
            .conn_info()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "SMB connection was not negotiated"))?
            .negotiation;
        let chunk = (negotiation.max_read_size as usize)
            .min(negotiation.max_write_size as usize)
            .min(64 * 1024);
        if chunk == 0 {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "SMB transfer size is zero",
            ));
        }
        let mut buffer = vec![0; chunk];
        let mut offset = 0_u64;
        loop {
            let read = source
                .read_block(&mut buffer, offset, None, false)
                .await
                .map_err(new_std_io_error)?;
            if read == 0 {
                break;
            }
            let mut written = 0;
            while written < read {
                let size = target
                    .write_block(&buffer[written..read], offset + written as u64, None)
                    .await
                    .map_err(new_std_io_error)?;
                if size == 0 {
                    return Err(new_std_io_error(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "SMB server did not write any bytes",
                    )));
                }
                written += size;
            }
            offset += read as u64;
        }
        target.flush().await.map_err(new_std_io_error)?;
        let info = target
            .query_info::<FileAllInformation>()
            .await
            .map_err(parse_smb_error)?;
        to_metadata(
            info.basic.file_attributes,
            info.standard.end_of_file,
            info.basic.last_write_time,
        )
    }
}
