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
use super::core::CheckNameMode;
use super::core::CreateResponse;
use super::core::CreateType;
use super::core::UploadUrlResponse;
use crate::raw::*;
use crate::*;

pub struct AliyunDriveWriter {
    core: Arc<AliyunDriveCore>,

    _op: OpWrite,
    parent_file_id: String,
    name: String,

    file_id: Option<String>,
    upload_id: Option<String>,
    part_number: usize,
}

impl AliyunDriveWriter {
    pub fn new(core: Arc<AliyunDriveCore>, parent_file_id: &str, name: &str, op: OpWrite) -> Self {
        AliyunDriveWriter {
            core,
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

        let res = self
            .core
            .get_upload_url(file_id, upload_id, Some(self.part_number))
            .await?;
        let output: UploadUrlResponse =
            serde_json::from_reader(res.reader()).map_err(new_json_deserialize_error)?;

        let Some(upload_url) = output
            .part_info_list
            .as_ref()
            .and_then(|list| list.first())
            .map(|part_info| &part_info.upload_url)
        else {
            return Err(Error::new(ErrorKind::Unexpected, "cannot find upload_url"));
        };

        if let Err(err) = self.core.upload(upload_url, bs).await {
            if err.kind() != ErrorKind::AlreadyExists {
                return Err(err);
            }
        };

        self.part_number += 1;

        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        let (Some(upload_id), Some(file_id)) = (self.upload_id.as_ref(), self.file_id.as_ref())
        else {
            return Ok(Metadata::default());
        };
        self.core.complete(file_id, upload_id).await?;
        Ok(Metadata::default())
    }

    async fn abort(&mut self) -> Result<()> {
        let Some(file_id) = self.file_id.as_ref() else {
            return Ok(());
        };
        self.core.delete_path(file_id).await
    }
}
