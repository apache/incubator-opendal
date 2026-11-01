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
use quick_xml::de;

use super::core::AzblobCore;
use super::core::ErrorContext;
use super::core::ListBlobsOutput;
use super::core::parse_error;
use opendal_core::raw::*;
use opendal_core::*;

pub struct AzblobLister {
    core: Arc<AzblobCore>,
    ctx: OperationContext,

    path: String,
    delimiter: &'static str,
    limit: Option<usize>,
}

impl AzblobLister {
    pub fn new(
        core: Arc<AzblobCore>,
        ctx: OperationContext,
        path: String,
        recursive: bool,
        limit: Option<usize>,
    ) -> Self {
        let delimiter = if recursive { "" } else { "/" };

        Self {
            core,
            ctx,
            path,
            delimiter,
            limit,
        }
    }
}

impl oio::PageList for AzblobLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self
            .core
            .azblob_list_blobs(
                &self.ctx,
                &self.path,
                &ctx.token,
                self.delimiter,
                self.limit,
            )
            .await?;

        if resp.status() != http::StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListBlobs")),
                resp,
            ));
        }

        let bs = resp.into_body();

        let output: ListBlobsOutput =
            de::from_reader(bs.reader()).map_err(new_xml_deserialize_error)?;

        // Try our best to check whether this list is done.
        //
        // - Check `next_marker`
        if let Some(next_marker) = output.next_marker.as_ref() {
            ctx.done = next_marker.is_empty();
        };
        ctx.token = output.next_marker.clone().unwrap_or_default();

        let prefixes = output.blobs.blob_prefix;

        for prefix in prefixes {
            let de = oio::Entry::new(
                &build_rel_path(&self.core.root, &prefix.name),
                MetadataBuilder::dir().build(),
            );

            ctx.entries.push_back(de)
        }

        for object in output.blobs.blob {
            let mut path = build_rel_path(&self.core.root, &object.name);
            if path.is_empty() {
                path = "/".to_string();
            }

            let mut meta = if path.ends_with('/') {
                MetadataBuilder::dir()
            } else {
                MetadataBuilder::file(object.properties.content_length)
            };
            meta.etag(format!("\"{}\"", object.properties.etag.as_str()))
                .content_md5(object.properties.content_md5)
                .content_type(object.properties.content_type)
                .last_modified(Timestamp::parse_rfc2822(
                    object.properties.last_modified.as_str(),
                )?);

            let de = oio::Entry::with(path, meta.build());
            ctx.entries.push_back(de);
        }

        Ok(())
    }
}
