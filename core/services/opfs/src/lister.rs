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
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::core::OpfsCore;
use super::core::*;
use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub struct OpfsLister {
    core: Arc<OpfsCore>,
    iter: Option<SendWrapper<js_sys::AsyncIterator>>,
    path: String,
}

impl OpfsLister {
    pub fn new(core: Arc<OpfsCore>, path: String) -> Self {
        // Entry paths must not start with '/'.
        // For root listing, path is "/" — normalize to "".
        let path = if path == "/" { String::new() } else { path };
        Self {
            core,
            iter: None,
            path,
        }
    }

    async fn init_iter(&mut self) -> Result<()> {
        if self.iter.is_some() {
            return Ok(());
        }

        let p = build_abs_path(&self.core.root, &self.path);
        let dir = get_directory_handle(&p, false).await?;
        self.iter = Some(SendWrapper::new(dir.entries()));

        Ok(())
    }
}

impl oio::List for OpfsLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.init_iter().await?;

        let iter = self
            .iter
            .as_ref()
            .expect("opfs list iterator must be initialized");
        let result = JsFuture::from(iter.next().map_err(parse_js_error)?)
            .await
            .map_err(parse_js_error)?;

        let done = js_sys::Reflect::get(&result, &"done".into())
            .unwrap_or(JsValue::TRUE)
            .as_bool()
            .unwrap_or(true);
        if done {
            return Ok(None);
        }

        let value = js_sys::Reflect::get(&result, &"value".into()).map_err(parse_js_error)?;
        let pair: js_sys::Array = value.unchecked_into();
        let name = pair.get(0).as_string().unwrap_or_default();
        let handle = pair.get(1);

        let kind = js_sys::Reflect::get(&handle, &"kind".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();

        let (entry_path, metadata) = if kind == "directory" {
            (format!("{}{}/", self.path, name), MetadataBuilder::dir())
        } else {
            (format!("{}{}", self.path, name), MetadataBuilder::unknown())
        };

        Ok(Some(oio::Entry::new(&entry_path, metadata.build())))
    }
}
