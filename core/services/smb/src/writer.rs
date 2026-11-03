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

use bytes::Buf;
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

use super::core::RuntimeResource;
use super::core::SmbCore;
use super::core::to_metadata;
use super::error::parse_smb_error;

pub(super) struct SmbWriter {
    core: Arc<SmbCore>,
    path: String,
    op: OpWrite,
    inner: Option<SmbWriteHandle>,
}

struct SmbWriteHandle {
    file: RuntimeResource<File>,
    offset: u64,
    max_write_size: usize,
}

impl SmbWriter {
    pub fn new(core: Arc<SmbCore>, path: &str, op: OpWrite) -> Self {
        Self {
            core,
            path: path.to_string(),
            op,
            inner: None,
        }
    }

    async fn inner(&mut self) -> Result<&mut SmbWriteHandle> {
        if self.inner.is_none() {
            let target = self.core.path(&self.path)?;
            if let Some((parent, _)) = self.path.rsplit_once('/') {
                self.core.create_dir(parent).await?;
            } else {
                self.core.create_dir("/").await?;
            }
            let client = self.core.client().await?;
            let connection = client
                .get_connection(self.core.share.server())
                .await
                .map_err(parse_smb_error)?;
            // A request of at most 64 KiB consumes one SMB credit, so a large
            // negotiated transfer size cannot exhaust the connection's credits.
            let max_write_size = (connection
                .conn_info()
                .ok_or_else(|| {
                    Error::new(ErrorKind::Unexpected, "SMB connection was not negotiated")
                })?
                .negotiation
                .max_write_size as usize)
                .min(64 * 1024);
            let args = FileCreateArgs {
                disposition: if self.op.if_not_exists() {
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
            let resource = client.create_file(&target, &args).await.map_err(|e| {
                let error = parse_smb_error(e);
                if self.op.if_not_exists() && error.kind() == ErrorKind::AlreadyExists {
                    Error::new(ErrorKind::ConditionNotMatch, "file already exists")
                        .set_source(error)
                } else {
                    error
                }
            })?;
            let file = match resource {
                Resource::File(file) => file,
                Resource::Directory(_) => {
                    return Err(Error::new(
                        ErrorKind::IsADirectory,
                        "cannot write a directory",
                    ));
                }
                Resource::Pipe(_) => {
                    return Err(Error::new(
                        ErrorKind::Unsupported,
                        "SMB pipes are not supported",
                    ));
                }
            };
            self.inner = Some(SmbWriteHandle {
                file: RuntimeResource::new(file),
                offset: 0,
                max_write_size,
            });
        }
        Ok(self.inner.as_mut().expect("writer is initialized"))
    }
}

impl oio::Write for SmbWriter {
    async fn write(&mut self, mut buffer: Buffer) -> Result<()> {
        let handle = self.inner().await?;
        while buffer.has_remaining() {
            let chunk = buffer.chunk();
            let size = chunk.len().min(handle.max_write_size);
            let written = handle
                .file
                .write_block(&chunk[..size], handle.offset, None)
                .await
                .map_err(new_std_io_error)?;
            if written == 0 {
                return Err(new_std_io_error(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "SMB server did not write any bytes",
                )));
            }
            buffer.advance(written);
            handle.offset += written as u64;
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let handle = self.inner().await?;
        handle.file.flush().await.map_err(new_std_io_error)?;
        let info = handle.file.query_info::<FileAllInformation>().await;
        handle.file.close().await.map_err(parse_smb_error)?;
        let info = info.map_err(parse_smb_error)?;
        to_metadata(
            info.basic.file_attributes,
            info.standard.end_of_file,
            info.basic.last_write_time,
        )
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "SMB writes do not support abort",
        ))
    }
}
