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

use super::core::AlluxioCore;
use opendal_core::ErrorKind;
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::oio::Entry;
use opendal_core::raw::*;

pub struct AlluxioLister {
    core: Arc<AlluxioCore>,
    ctx: OperationContext,

    path: String,
}

impl AlluxioLister {
    pub(super) fn new(core: Arc<AlluxioCore>, ctx: OperationContext, path: &str) -> Self {
        AlluxioLister {
            core,
            ctx,
            path: path.to_string(),
        }
    }
}

impl oio::PageList for AlluxioLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let result = self.core.list_status(&self.ctx, &self.path).await;

        match result {
            Ok(file_infos) => {
                ctx.done = true;

                for file_info in file_infos {
                    let path: String = file_info.path.clone();
                    let path = if file_info.folder {
                        format!("{path}/")
                    } else {
                        path
                    };
                    ctx.entries.push_back(Entry::new(
                        &build_rel_path(&self.core.root, &path),
                        file_info.try_into()?,
                    ));
                }

                Ok(())
            }
            Err(e) => {
                if e.kind() == ErrorKind::NotFound {
                    ctx.done = true;
                    return Ok(());
                }
                Err(e)
            }
        }
    }
}
