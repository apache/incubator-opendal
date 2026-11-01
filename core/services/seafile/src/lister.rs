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

use super::core::SeafileCore;
use opendal_core::raw::oio::Entry;
use opendal_core::raw::*;
use opendal_core::*;

pub struct SeafileLister {
    core: Arc<SeafileCore>,
    ctx: OperationContext,

    path: String,
}

impl SeafileLister {
    pub(super) fn new(core: Arc<SeafileCore>, ctx: OperationContext, path: &str) -> Self {
        SeafileLister {
            core,
            ctx,
            path: path.to_string(),
        }
    }
}

impl oio::PageList for SeafileLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let list_response = self.core.list(&self.ctx, &self.path).await?;
        match list_response.infos {
            Some(infos) => {
                // add path itself
                ctx.entries.push_back(Entry::new(
                    self.path.as_str(),
                    MetadataBuilder::dir().build(),
                ));

                for info in infos {
                    if !info.name.is_empty() {
                        let rel_path = build_rel_path(
                            &self.core.root,
                            &format!("{}{}", list_response.rooted_abs_path, info.name),
                        );

                        let entry = if info.type_field == "file" {
                            let size = info.size.ok_or_else(|| {
                                Error::new(
                                    ErrorKind::Unexpected,
                                    "seafile list response does not contain file size",
                                )
                            })?;
                            let mut meta = MetadataBuilder::file(size);
                            meta.last_modified(Timestamp::from_second(info.mtime)?);
                            Entry::new(&rel_path, meta.build())
                        } else {
                            let path = format!("{rel_path}/");
                            Entry::new(&path, MetadataBuilder::dir().build())
                        };

                        ctx.entries.push_back(entry);
                    }
                }

                ctx.done = true;

                Ok(())
            }
            // return nothing when not exist
            None => {
                ctx.done = true;
                Ok(())
            }
        }
    }
}
