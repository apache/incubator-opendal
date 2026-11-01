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

use bytes::Bytes;
use futures::TryStreamExt;
use futures::stream::BoxStream;
use object_store::ObjectStore;
use object_store::path::Path as ObjectStorePath;

use opendal::raw::*;
use opendal::*;

use super::core::format_metadata;
use super::core::parse_op_read;
use super::error::parse_error;

/// ObjectStore reader
pub struct ObjectStoreReader {
    store: Arc<dyn ObjectStore + 'static>,
    path: String,
    args: OpRead,
}

impl ObjectStoreReader {
    pub(crate) fn new(store: Arc<dyn ObjectStore + 'static>, path: &str, args: OpRead) -> Self {
        Self {
            store,
            path: path.to_string(),
            args,
        }
    }
}

impl oio::StreamRead for ObjectStoreReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let path = ObjectStorePath::from(self.path.as_str());
        let opts = parse_op_read(&self.args, range)?;
        let result = self
            .store
            .get_opts(&path, opts)
            .await
            .map_err(parse_error)?;
        let rp = RpRead::new(format_metadata(&result.meta));
        let stream = ObjectStoreReadStream::new(result.into_stream());

        Ok((rp, Box::new(stream) as Box<dyn oio::ReadStreamDyn>))
    }
}

/// ObjectStore read stream
pub struct ObjectStoreReadStream {
    bytes_stream: BoxStream<'static, object_store::Result<Bytes>>,
}

impl ObjectStoreReadStream {
    fn new(bytes_stream: BoxStream<'static, object_store::Result<Bytes>>) -> Self {
        Self { bytes_stream }
    }
}

// ObjectStoreReadStream is safe to share between threads, because the `read()` method requires
// `&mut self`.
unsafe impl Sync for ObjectStoreReadStream {}

impl oio::ReadStream for ObjectStoreReadStream {
    async fn read(&mut self) -> Result<Buffer> {
        let bs = self.bytes_stream.try_next().await.map_err(parse_error)?;
        Ok(bs.map(Buffer::from).unwrap_or_default())
    }
}
