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
use super::config::OpfsConfig;
use super::core::OpfsCore;
use super::core::*;
use super::deleter::OpfsDeleter;
use super::lister::OpfsLister;
use super::writer::OpfsWriter;
use opendal_core::raw::*;
use opendal_core::*;
use send_wrapper::SendWrapper;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::File;
use web_sys::FileSystemFileHandle;

pub struct OpfsReadStream {
    handle: SendWrapper<FileSystemFileHandle>,
    range: BytesRange,
    done: bool,
}

impl OpfsReadStream {
    pub fn new(handle: FileSystemFileHandle, range: BytesRange) -> Self {
        Self {
            handle: SendWrapper::new(handle),
            range,
            done: false,
        }
    }
}

impl oio::ReadStream for OpfsReadStream {
    async fn read(&mut self) -> Result<Buffer> {
        if self.done {
            return Ok(Buffer::new());
        }
        self.done = true;

        let file: web_sys::File = JsFuture::from(self.handle.get_file())
            .await
            .and_then(JsCast::dyn_into)
            .map_err(parse_js_error)?;

        let blob: &web_sys::Blob = file.as_ref();
        let blob = if self.range.is_full() {
            blob.clone()
        } else {
            let offset = self.range.offset() as f64;
            let end = match self.range.size() {
                Some(size) => offset + size as f64,
                None => blob.size(),
            };
            blob.slice_with_f64_and_f64(offset, end)
                .map_err(parse_js_error)?
        };

        let array_buffer = JsFuture::from(blob.array_buffer())
            .await
            .map_err(parse_js_error)?;
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);

        Ok(Buffer::from(uint8_array.to_vec()))
    }
}

/// Reader returned by this backend.
pub struct OpfsReader {
    backend: OpfsBackend,
    path: String,
}

impl OpfsReader {
    pub(super) fn new(backend: OpfsBackend, path: &str, _: OpRead) -> Self {
        Self {
            backend,
            path: path.to_string(),
        }
    }
}

impl oio::StreamRead for OpfsReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let backend = &self.backend;
        let path = self.path.as_str();

        let p = build_abs_path(&backend.core.root, path);
        let handle = get_file_handle(&p, false).await?;
        let rp = RpRead::default();
        let stream = OpfsReadStream::new(handle, range);

        Ok((rp, Box::new(stream) as Box<dyn oio::ReadStreamDyn>))
    }
}
