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

use std::fmt::Debug;
use std::sync::Arc;

use backon::Retryable;
use bytes::Buf;
use http::Response;
use http::StatusCode;

use super::core::*;
use super::error::*;
use super::lister::DropboxLister;
use super::writer::DropboxWriter;
use crate::raw::*;
use crate::*;

#[derive(Clone, Debug)]
pub struct DropboxBackend {
    pub core: Arc<DropboxCore>,
}

impl Access for DropboxBackend {
    type Reader = HttpBody;
    type Writer = oio::OneShotWriter<DropboxWriter>;
    type Lister = oio::PageLister<DropboxLister>;
    type BlockingReader = ();
    type BlockingWriter = ();
    type BlockingLister = ();

    fn info(&self) -> Arc<AccessorInfo> {
        let mut ma = AccessorInfo::default();
        ma.set_scheme(Scheme::Dropbox)
            .set_root(&self.core.root)
            .set_native_capability(Capability {
                stat: true,

                read: true,

                write: true,

                create_dir: true,

                delete: true,

                list: true,
                list_with_recursive: true,

                copy: true,

                rename: true,

                batch: true,
                batch_delete: true,

                ..Default::default()
            });
        ma.into()
    }

    async fn create_dir(&self, path: &str, _args: OpCreateDir) -> Result<RpCreateDir> {
        // Check if the folder already exists.
        let resp = self.core.dropbox_get_metadata(path).await?;
        if StatusCode::OK == resp.status() {
            let bytes = resp.into_body();
            let decoded_response: DropboxMetadataResponse =
                serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;
            if "folder" == decoded_response.tag {
                return Ok(RpCreateDir::default());
            }
            if "file" == decoded_response.tag {
                return Err(Error::new(
                    ErrorKind::NotADirectory,
                    format!("it's not a directory {}", path),
                ));
            }
        }

        // Dropbox has very, very, very strong limitation on the create_folder requests.
        //
        // Let's try our best to make sure it won't failed for rate limited issues.
        let res = { || self.core.dropbox_create_folder(path) }
            .retry(*BACKOFF)
            .when(|e| e.is_temporary())
            .await
            // Set this error to permanent to avoid retrying.
            .map_err(|e| e.set_permanent())?;

        Ok(res)
    }

    async fn stat(&self, path: &str, _: OpStat) -> Result<RpStat> {
        let resp = self.core.dropbox_get_metadata(path).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let bytes = resp.into_body();
                let decoded_response: DropboxMetadataResponse =
                    serde_json::from_reader(bytes.reader()).map_err(new_json_deserialize_error)?;
                let entry_mode: EntryMode = match decoded_response.tag.as_str() {
                    "file" => EntryMode::FILE,
                    "folder" => EntryMode::DIR,
                    _ => EntryMode::Unknown,
                };

                let mut metadata = Metadata::new(entry_mode);
                // Only set last_modified and size if entry_mode is FILE, because Dropbox API
                // returns last_modified and size only for files.
                // FYI: https://www.dropbox.com/developers/documentation/http/documentation#files-get_metadata
                if entry_mode == EntryMode::FILE {
                    let date_utc_last_modified =
                        parse_datetime_from_rfc3339(&decoded_response.client_modified)?;
                    metadata.set_last_modified(date_utc_last_modified);

                    if let Some(size) = decoded_response.size {
                        metadata.set_content_length(size);
                    } else {
                        return Err(Error::new(
                            ErrorKind::Unexpected,
                            format!("no size found for file {}", path),
                        ));
                    }
                }
                Ok(RpStat::new(metadata))
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let resp = self.core.dropbox_get(path, args.range(), &args).await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                Ok((RpRead::default(), resp.into_body()))
            }
            _ => {
                let (part, mut body) = resp.into_parts();
                let buf = body.to_buffer().await?;
                Err(parse_error(Response::from_parts(part, buf)).await?)
            }
        }
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        Ok((
            RpWrite::default(),
            oio::OneShotWriter::new(DropboxWriter::new(
                self.core.clone(),
                args,
                String::from(path),
            )),
        ))
    }

    async fn delete(&self, path: &str, _: OpDelete) -> Result<RpDelete> {
        let resp = self.core.dropbox_delete(path).await?;

        let status = resp.status();

        match status {
            StatusCode::OK => Ok(RpDelete::default()),
            _ => {
                let err = parse_error(resp).await?;
                match err.kind() {
                    ErrorKind::NotFound => Ok(RpDelete::default()),
                    _ => Err(err),
                }
            }
        }
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        Ok((
            RpList::default(),
            oio::PageLister::new(DropboxLister::new(
                self.core.clone(),
                path.to_string(),
                args.recursive(),
                args.limit(),
            )),
        ))
    }

    async fn copy(&self, from: &str, to: &str, _: OpCopy) -> Result<RpCopy> {
        // Check if the from path exists.
        let resp = self.core.dropbox_get_metadata(from).await?;
        if StatusCode::OK != resp.status() {
            let err = parse_error(resp).await?;
            return Err(err);
        }

        // Check if the to path already exists.
        // We need delete it fistly due to Dropbox doesn't support overwrite.
        let resp = self.core.dropbox_get_metadata(to).await?;
        if StatusCode::OK == resp.status() {
            let resp = self.core.dropbox_delete(to).await?;
            if StatusCode::OK != resp.status() {
                let err = parse_error(resp).await?;
                return Err(err);
            }
        }

        let resp = self.core.dropbox_copy(from, to).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => Ok(RpCopy::default()),
            _ => {
                let err = parse_error(resp).await?;
                Err(err)
            }
        }
    }

    async fn rename(&self, from: &str, to: &str, _: OpRename) -> Result<RpRename> {
        // Check if the from path exists.
        let resp = self.core.dropbox_get_metadata(from).await?;
        if StatusCode::OK != resp.status() {
            let err = parse_error(resp).await?;
            return Err(err);
        }

        // Check if the to path already exists.
        // We need delete it fistly due to Dropbox doesn't support overwrite.
        let resp = self.core.dropbox_get_metadata(to).await?;
        if StatusCode::OK == resp.status() {
            let resp = self.core.dropbox_delete(to).await?;
            if StatusCode::OK != resp.status() {
                let err = parse_error(resp).await?;
                return Err(err);
            }
        }

        let resp = self.core.dropbox_move(from, to).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => Ok(RpRename::default()),
            _ => {
                let err = parse_error(resp).await?;
                Err(err)
            }
        }
    }

    async fn batch(&self, args: OpBatch) -> Result<RpBatch> {
        let ops = args.into_operation();
        if ops.len() > 1000 {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "dropbox services only allow delete up to 1000 keys at once",
            )
            .with_context("length", ops.len().to_string()));
        }

        let paths = ops.into_iter().map(|(p, _)| p).collect::<Vec<_>>();

        let resp = self.core.dropbox_delete_batch(paths).await?;
        if resp.status() != StatusCode::OK {
            return Err(parse_error(resp).await?);
        }

        let bs = resp.into_body();
        let decoded_response: DropboxDeleteBatchResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        match decoded_response.tag.as_str() {
            "complete" => {
                let entries = decoded_response.entries.unwrap_or_default();
                let results = self.core.handle_batch_delete_complete_result(entries);
                Ok(RpBatch::new(results))
            }
            "async_job_id" => {
                let job_id = decoded_response
                    .async_job_id
                    .expect("async_job_id should be present");
                let res = { || self.core.dropbox_delete_batch_check(job_id.clone()) }
                    .retry(*BACKOFF)
                    .when(|e| e.is_temporary())
                    .await?;

                Ok(res)
            }
            _ => Err(Error::new(
                ErrorKind::Unexpected,
                format!(
                    "delete batch failed with unexpected tag {}",
                    decoded_response.tag
                ),
            )),
        }
    }
}
