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

use bytes::Buf;
use bytes::Bytes;
use http::StatusCode;

use super::backend::OnedriveBackend;
use super::error::parse_error;
use super::graph_model::OneDriveUploadSessionCreationRequestBody;
use super::graph_model::OneDriveUploadSessionCreationResponseBody;
use crate::raw::*;
use crate::*;

pub struct OneDriveWriter {
    backend: OnedriveBackend,

    op: OpWrite,
    path: String,
}

impl OneDriveWriter {
    const MAX_SIMPLE_SIZE: usize = 4 * 1024 * 1024;
    // If your app splits a file into multiple byte ranges, the size of each byte range MUST be a multiple of 320 KiB (327,680 bytes). Using a fragment size that does not divide evenly by 320 KiB will result in errors committing some files.
    // https://learn.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_createuploadsession?view=odsp-graph-online#upload-bytes-to-the-upload-session
    const CHUNK_SIZE_FACTOR: usize = 327_680;
    pub fn new(backend: OnedriveBackend, op: OpWrite, path: String) -> Self {
        OneDriveWriter { backend, op, path }
    }
}

impl oio::OneShotWrite for OneDriveWriter {
    async fn write_once(&self, bs: Buffer) -> Result<Metadata> {
        let size = bs.len();

        if size <= Self::MAX_SIMPLE_SIZE {
            self.write_simple(bs).await?;
        } else {
            self.write_chunked(bs.to_bytes()).await?;
        }

        Ok(Metadata::default())
    }
}

impl OneDriveWriter {
    async fn write_simple(&self, bs: Buffer) -> Result<()> {
        let resp = self
            .backend
            .onedrive_upload_simple(&self.path, Some(bs.len()), &self.op, bs)
            .await?;

        let status = resp.status();

        match status {
            // Typical response code: 201 Created
            // Reference: https://learn.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_put_content?view=odsp-graph-online#response
            StatusCode::CREATED | StatusCode::OK => Ok(()),
            _ => Err(parse_error(resp)),
        }
    }

    pub(crate) async fn write_chunked(&self, total_bytes: Bytes) -> Result<()> {
        // Upload large files via sessions: https://learn.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_createuploadsession?view=odsp-graph-online#upload-bytes-to-the-upload-session
        // 1. Create an upload session
        // 2. Upload the bytes of each chunk
        // 3. Commit the session

        let session_response = self.create_upload_session().await?;

        let mut offset = 0;

        let iter = total_bytes.chunks(OneDriveWriter::CHUNK_SIZE_FACTOR);

        for chunk in iter {
            let mut end = offset + OneDriveWriter::CHUNK_SIZE_FACTOR;
            if end > total_bytes.len() {
                end = total_bytes.len();
            }
            let total_len = total_bytes.len();
            let chunk_end = end - 1;

            let resp = self
                .backend
                .onedrive_chunked_upload(
                    &session_response.upload_url,
                    &OpWrite::default(),
                    offset,
                    chunk_end,
                    total_len,
                    Buffer::from(Bytes::copy_from_slice(chunk)),
                )
                .await?;

            let status = resp.status();

            match status {
                // Typical response code: 202 Accepted
                // Reference: https://learn.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_put_content?view=odsp-graph-online#response
                StatusCode::ACCEPTED | StatusCode::CREATED | StatusCode::OK => {}
                _ => return Err(parse_error(resp)),
            }

            offset += OneDriveWriter::CHUNK_SIZE_FACTOR;
        }

        Ok(())
    }

    async fn create_upload_session(&self) -> Result<OneDriveUploadSessionCreationResponseBody> {
        let file_name_from_path = self.path.split('/').next_back().ok_or_else(|| {
            Error::new(
                ErrorKind::Unexpected,
                "connection string must have AccountName",
            )
        })?;
        let url = format!(
            "{}/drive/root:{}:/createUploadSession",
            OnedriveBackend::BASE_URL,
            percent_encode_path(&self.path)
        );
        let body = OneDriveUploadSessionCreationRequestBody::new(file_name_from_path.to_string());

        let resp = self
            .backend
            .onedrive_create_upload_session(&url, body)
            .await?;

        let status = resp.status();

        match status {
            // Reference: https://learn.microsoft.com/en-us/onedrive/developer/rest-api/api/driveitem_createuploadsession?view=odsp-graph-online#response
            StatusCode::OK => {
                let bs = resp.into_body();
                let result: OneDriveUploadSessionCreationResponseBody =
                    serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;
                Ok(result)
            }
            _ => Err(parse_error(resp)),
        }
    }
}
