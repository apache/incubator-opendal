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
pub struct MongodbReader {
    backend: MongodbBackend,
    path: String,
}

impl MongodbReader {
    pub(super) fn new(backend: MongodbBackend, path: &str, _: OpRead) -> Self {
        Self {
            backend,
            path: path.to_string(),
        }
    }
}

impl oio::StreamRead for MongodbReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();
        let p = build_abs_path(&backend.root, path);
        let bs = match backend.core.get(&p).await? {
            Some(bs) => bs,
            None => {
                return Err(Error::new(ErrorKind::NotFound, "kv not found in mongodb"));
            }
        };
        let content = bs.slice(range.to_content_range(bs.len())?);
        let metadata = {
            let metadata = MetadataBuilder::file(bs.len() as u64);
            metadata.build()
        };
        Ok((
            RpRead::new(metadata),
            Box::new(content) as Box<dyn oio::ReadStreamDyn>,
        ))
    }
}
