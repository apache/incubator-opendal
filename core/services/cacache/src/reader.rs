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
use opendal_core::raw::*;
use opendal_core::*;

/// Reader returned by this backend.
pub struct CacacheReader {
    backend: CacacheBackend,
    path: String,
}

impl CacacheReader {
    pub(super) fn new(backend: CacacheBackend, path: &str, _: OpRead) -> Self {
        Self {
            backend,
            path: path.to_string(),
        }
    }
}

impl oio::StreamRead for CacacheReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let data = backend.core.get(path).await?;

        match data {
            Some(bytes) => {
                let content_length = bytes.len() as u64;
                let buffer = Buffer::from(bytes.slice(range.to_content_range(bytes.len())?));
                let metadata = {
                    let metadata = MetadataBuilder::file(content_length);
                    metadata.build()
                };
                Ok((
                    RpRead::new(metadata),
                    Box::new(buffer) as Box<dyn oio::ReadStreamDyn>,
                ))
            }
            None => Err(Error::new(ErrorKind::NotFound, "entry not found")),
        }
    }
}
