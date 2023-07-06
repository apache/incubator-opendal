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

use async_trait::async_trait;
use http::StatusCode;

use super::core::DropboxCore;
use super::error::parse_error;
use super::response::DropboxMetadataResponse;
use super::writer::DropboxWriter;
use crate::raw::*;
use crate::*;

#[derive(Clone, Debug)]
pub struct DropboxBackend {
    pub core: Arc<DropboxCore>,
}

#[async_trait]
impl Accessor for DropboxBackend {
    type Reader = IncomingAsyncBody;
    type BlockingReader = ();
    type Writer = DropboxWriter;
    type BlockingWriter = ();
    type Pager = ();
    type BlockingPager = ();
    type Appender = ();

    fn info(&self) -> AccessorInfo {
        let mut ma = AccessorInfo::default();
        ma.set_scheme(Scheme::Dropbox)
            .set_root(&self.core.root)
            .set_capability(Capability {
                stat: true,

                read: true,

                write: true,

                create_dir: true,

                delete: true,

                ..Default::default()
            });
        ma
    }

    async fn create_dir(&self, path: &str, _args: OpCreateDir) -> Result<RpCreateDir> {
        let resp = self.core.dropbox_create_folder(path).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => Ok(RpCreateDir::default()),
            _ => {
                let err = parse_error(resp).await?;
                match err.kind() {
                    ErrorKind::AlreadyExists => Ok(RpCreateDir::default()),
                    _ => Err(err),
                }
            },
        }
    }

    async fn read(&self, path: &str, _args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let resp = self.core.dropbox_get(path).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let meta = parse_into_metadata(path, resp.headers())?;
                Ok((RpRead::with_metadata(meta), resp.into_body()))
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        if args.content_length().is_none() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "write without content length is not supported",
            ));
        }
        Ok((
            RpWrite::default(),
            DropboxWriter::new(self.core.clone(), args, String::from(path)),
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

    async fn stat(&self, path: &str, _: OpStat) -> Result<RpStat> {
        if path == "/" {
            return Ok(RpStat::new(Metadata::new(EntryMode::DIR)));
        }
        let resp = self.core.dropbox_get_metadata(path).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let bytes = resp.into_body().bytes().await?;
                let decoded_response = serde_json::from_slice::<DropboxMetadataResponse>(&bytes)
                    .map_err(new_json_deserialize_error)?;
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
                    let last_modified = decoded_response.client_modified;
                    let date_utc_last_modified = parse_datetime_from_rfc3339(&last_modified)?;
                    metadata.set_last_modified(date_utc_last_modified);
                    if decoded_response.size.is_some() {
                        let size = decoded_response.size.unwrap();
                        metadata.set_content_length(size);
                    }
                }
                Ok(RpStat::new(metadata))
            }
            StatusCode::NOT_FOUND if path.ends_with('/') => {
                Ok(RpStat::new(Metadata::new(EntryMode::DIR)))
            }
            _ => Err(parse_error(resp).await?),
        }
    }
}
