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

use bytes::Buf;
use http::StatusCode;
use serde::Deserialize;

use super::core::parse_error;
use super::core::{ErrorContext, IpmfsCore};
use opendal_core::EntryMode;
use opendal_core::ErrorKind;
use opendal_core::MetadataBuilder;
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::*;

pub struct IpmfsLister {
    core: Arc<IpmfsCore>,
    ctx: OperationContext,
    root: String,
    path: String,
}

impl IpmfsLister {
    pub fn new(core: Arc<IpmfsCore>, ctx: OperationContext, root: &str, path: &str) -> Self {
        Self {
            core,
            ctx,
            root: root.to_string(),
            path: path.to_string(),
        }
    }
}

impl oio::PageList for IpmfsLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self.core.ipmfs_ls(&self.ctx, &self.path).await?;

        if resp.status() != StatusCode::OK {
            let err = parse_error(ErrorContext::new(ServiceOperation("FilesLs")), resp);
            if matches!(err.kind(), ErrorKind::NotFound) {
                // treat as empty listing
                ctx.done = true;
                return Ok(());
            }
            return Err(err);
        }

        // Add current directory entry when processing the first page
        if ctx.token.is_empty() && !ctx.done {
            let path = build_abs_path(&self.root, self.path.as_str());
            let path = build_rel_path(&self.root, &path);

            ctx.entries
                .push_back(oio::Entry::new(&path, MetadataBuilder::dir().build()));
        }

        let bs = resp.into_body();
        let entries_body: IpfsLsResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        // Mark dir stream has been consumed.
        ctx.done = true;

        for object in entries_body.entries.unwrap_or_default() {
            let prefix = if self.path == "/" {
                ""
            } else {
                self.path.as_str()
            };
            let path = match object.mode() {
                EntryMode::FILE => format!("{prefix}{}", object.name),
                EntryMode::DIR => format!("{prefix}{}/", object.name),
                EntryMode::Unknown => unreachable!(),
            };

            ctx.entries.push_back(oio::Entry::new(&path, {
                let metadata = MetadataBuilder::file(object.size);
                metadata.build()
            }));
        }

        Ok(())
    }
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct IpfsLsResponseEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Type")]
    file_type: i64,
    #[serde(rename = "Size")]
    size: u64,
}

impl IpfsLsResponseEntry {
    /// ref: <https://github.com/ipfs/specs/blob/main/UNIXFS.md#data-format>
    ///
    /// ```protobuf
    /// enum DataType {
    ///     Raw = 0;
    ///     Directory = 1;
    ///     File = 2;
    ///     Metadata = 3;
    ///     Symlink = 4;
    ///     HAMTShard = 5;
    /// }
    /// ```
    fn mode(&self) -> EntryMode {
        match &self.file_type {
            1 => EntryMode::DIR,
            0 | 2 => EntryMode::FILE,
            _ => EntryMode::Unknown,
        }
    }
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
struct IpfsLsResponse {
    #[serde(rename = "Entries")]
    entries: Option<Vec<IpfsLsResponseEntry>>,
}
