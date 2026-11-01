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

use super::backend::*;
use super::core::{ErrorContext, parse_error};
use goosefs_sdk::io::GoosefsFileReader as SdkReader;
use opendal_core::raw::*;
use opendal_core::*;

pub struct GoosefsReadStream {
    inner: SdkReader,
    // The SDK may reopen its stream after EOF; keep OpenDAL's EOF terminal.
    done: bool,
}

impl oio::ReadStream for GoosefsReadStream {
    async fn read(&mut self) -> Result<Buffer> {
        if self.done {
            return Ok(Buffer::new());
        }

        match self
            .inner
            .read_next_block()
            .await
            .map_err(|err| parse_error(ErrorContext::new(ServiceOperation("ReadNextBlock")), err))?
        {
            Some(block) => Ok(Buffer::from(block)),
            None => {
                self.done = true;
                Ok(Buffer::new())
            }
        }
    }
}

/// Reader returned by this backend.
pub struct GoosefsReader {
    backend: GoosefsBackend,
    path: String,
}

impl GoosefsReader {
    pub(super) fn new(backend: GoosefsBackend, path: &str, _: OpRead) -> Self {
        Self {
            backend,
            path: path.to_string(),
        }
    }
}

impl oio::StreamRead for GoosefsReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();

        let inner = match (range.offset(), range.size()) {
            (0, None) => backend.core.open_reader(path).await?,
            (offset, Some(length)) => backend.core.open_range_reader(path, offset, length).await?,
            (offset, None) => {
                let file_info = backend.core.get_status(path).await?;
                let content_length = backend
                    .core
                    .file_info_to_metadata(&file_info)?
                    .content_length();
                backend
                    .core
                    .open_range_reader(path, offset, content_length.saturating_sub(offset))
                    .await?
            }
        };
        let metadata = backend.core.file_info_to_metadata(inner.file_info())?;
        let stream = GoosefsReadStream { inner, done: false };

        Ok((
            RpRead::new(metadata),
            Box::new(stream) as Box<dyn oio::ReadStreamDyn>,
        ))
    }
}
