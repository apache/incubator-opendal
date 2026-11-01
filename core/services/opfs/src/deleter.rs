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

use wasm_bindgen_futures::JsFuture;
use web_sys::FileSystemRemoveOptions;

use super::core::OpfsCore;
use super::core::*;
use super::core::*;
use opendal_core::raw::*;
use opendal_core::*;

pub struct OpfsDeleter {
    core: Arc<OpfsCore>,
}

impl OpfsDeleter {
    pub fn new(core: Arc<OpfsCore>) -> Self {
        Self { core }
    }
}

impl oio::OneShotDelete for OpfsDeleter {
    async fn delete_once(&self, path: String, args: OpDelete) -> Result<()> {
        let p = build_abs_path(&self.core.root, &path);
        let (dir, name) = get_parent_dir_and_name(&p, false).await?;

        let opt = FileSystemRemoveOptions::new();
        opt.set_recursive(args.recursive());

        match JsFuture::from(dir.remove_entry_with_options(name, &opt)).await {
            Ok(_) => Ok(()),
            Err(e) => {
                let err = parse_js_error(e);
                // Deleting a non-existent entry is not an error.
                if err.kind() == ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(err)
                }
            }
        }
    }
}
