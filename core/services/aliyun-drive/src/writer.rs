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

use super::core::AliyunDriveCore;
use super::core::AliyunDriveFile;
use super::core::CheckNameMode;
use super::core::CreateResponse;
use super::core::CreateType;
use opendal_core::raw::*;
use opendal_core::*;

pub struct AliyunDriveWriter {
    core: Arc<AliyunDriveCore>,
    ctx: OperationContext,

    _op: OpWrite,
    parent_file_id: String,
    name: String,

    file_id: Option<String>,
    upload_id: Option<String>,
    part_number: usize,
}

pub struct AliyunDriveLazyWriter {
    core: Arc<AliyunDriveCore>,
    ctx: OperationContext,
    path: String,
    args: OpWrite,
    inner: Option<AliyunDriveWriter>,
}

impl AliyunDriveLazyWriter {
    pub fn new(
        core: Arc<AliyunDriveCore>,
        ctx: OperationContext,
        path: String,
        args: OpWrite,
    ) -> Self {
        Self {
            core,
            ctx,
            path,
            args,
            inner: None,
        }
    }

    async fn inner(&mut self) -> Result<&mut AliyunDriveWriter> {
        if self.inner.is_none() {
            let parent_path = get_parent(&self.path);
            let parent_file_id = self.core.ensure_dir_exists(&self.ctx, parent_path).await?;

            // write can overwrite
            match self.core.get_by_path(&self.ctx, &self.path).await {
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(err),
                Ok(res) => {
                    let file: AliyunDriveFile =
                        serde_json::from_reader(res.reader()).map_err(new_json_serialize_error)?;
                    self.core.delete_path(&self.ctx, &file.file_id).await?;
                }
            };

            self.inner = Some(AliyunDriveWriter::new(
                self.core.clone(),
                self.ctx.clone(),
                &parent_file_id,
                get_basename(&self.path),
                self.args.clone(),
            ));
        }

        Ok(self
            .inner
            .as_mut()
            .expect("aliyun drive writer must be initialized"))
    }
}

impl oio::Write for AliyunDriveLazyWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner().await?.write(bs).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        match &mut self.inner {
            Some(w) => w.close().await,
            None => Ok(MetadataBuilder::unknown().build()),
        }
    }

    async fn abort(&mut self) -> Result<()> {
        match &mut self.inner {
            Some(w) => w.abort().await,
            None => Ok(()),
        }
    }
}

impl AliyunDriveWriter {
    pub fn new(
        core: Arc<AliyunDriveCore>,
        ctx: OperationContext,
        parent_file_id: &str,
        name: &str,
        op: OpWrite,
    ) -> Self {
        AliyunDriveWriter {
            core,
            ctx,
            _op: op,
            parent_file_id: parent_file_id.to_string(),
            name: name.to_string(),
            file_id: None,
            upload_id: None,
            part_number: 1, // must start from 1
        }
    }
}

impl oio::Write for AliyunDriveWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let (upload_id, file_id) = match (self.upload_id.as_ref(), self.file_id.as_ref()) {
            (Some(upload_id), Some(file_id)) => (upload_id, file_id),
            _ => {
                let res = self
                    .core
                    .create(
                        &self.ctx,
                        Some(&self.parent_file_id),
                        &self.name,
                        CreateType::File,
                        CheckNameMode::Refuse,
                    )
                    .await?;
                let output: CreateResponse =
                    serde_json::from_reader(res.reader()).map_err(new_json_deserialize_error)?;
                if output.exist.is_some_and(|x| x) {
                    return Err(Error::new(ErrorKind::AlreadyExists, "file exists"));
                }
                self.upload_id = output.upload_id;
                self.file_id = Some(output.file_id);
                (
                    self.upload_id.as_ref().expect("cannot find upload_id"),
                    self.file_id.as_ref().expect("cannot find file_id"),
                )
            }
        };

        if let Err(err) = self
            .core
            .upload(&self.ctx, file_id, upload_id, self.part_number, bs)
            .await
            && err.kind() != ErrorKind::AlreadyExists
        {
            return Err(err);
        };

        self.part_number += 1;

        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let (Some(upload_id), Some(file_id)) = (self.upload_id.as_ref(), self.file_id.as_ref())
        else {
            return Ok(MetadataBuilder::unknown().build());
        };

        self.core.complete(&self.ctx, file_id, upload_id).await?;
        Ok(MetadataBuilder::unknown().build())
    }

    async fn abort(&mut self) -> Result<()> {
        let Some(file_id) = self.file_id.as_ref() else {
            return Ok(());
        };
        self.core.delete_path(&self.ctx, file_id).await
    }
}
