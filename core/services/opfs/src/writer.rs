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

use send_wrapper::SendWrapper;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::FileSystemWritableFileStream;
use web_sys::WriteCommandType;
use web_sys::WriteParams;

use opendal_core::raw::*;
use opendal_core::*;

use super::core::OpfsCore;
use super::core::*;
use super::core::*;

pub struct OpfsWriter {
    core: Arc<OpfsCore>,
    path: String,
    stream: Option<SendWrapper<FileSystemWritableFileStream>>,
    bytes_written: u64,
}

impl OpfsWriter {
    pub fn new(core: Arc<OpfsCore>, path: String) -> Self {
        Self {
            core,
            path,
            stream: None,
            bytes_written: 0,
        }
    }

    async fn init_stream(&mut self) -> Result<()> {
        if self.stream.is_some() {
            return Ok(());
        }

        let p = build_abs_path(&self.core.root, &self.path);
        let handle = get_file_handle(&p, true).await?;
        let stream: FileSystemWritableFileStream = JsFuture::from(handle.create_writable())
            .await
            .and_then(JsCast::dyn_into)
            .map_err(parse_js_error)?;
        self.stream = Some(SendWrapper::new(stream));

        Ok(())
    }
}

impl oio::Write for OpfsWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.init_stream().await?;

        let bytes = bs.to_bytes();
        let params = WriteParams::new(WriteCommandType::Write);
        params.set_size(Some(bytes.len() as f64));
        let data: JsValue = js_sys::Uint8Array::from(bytes.as_ref()).into();
        params.set_data(&data);
        let stream = self
            .stream
            .as_ref()
            .expect("opfs writable stream must be initialized");
        JsFuture::from(
            stream
                .write_with_write_params(&params.into())
                .map_err(parse_js_error)?,
        )
        .await
        .map_err(parse_js_error)?;

        self.bytes_written += bytes.len() as u64;
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.init_stream().await?;

        if let Some(stream) = self.stream.take() {
            JsFuture::from(stream.close())
                .await
                .map_err(parse_js_error)?;
        }

        // We cannot set LastModified here - stream does not have such metadata
        Ok(MetadataBuilder::file(self.bytes_written).build())
    }

    async fn abort(&mut self) -> Result<()> {
        if let Some(stream) = self.stream.take() {
            JsFuture::from(stream.abort())
                .await
                .map_err(parse_js_error)?;
        }
        Ok(())
    }
}
