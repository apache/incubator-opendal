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

use crate::core::parse_error;
use crate::core::*;
use opendal_core::MetadataBuilder;
use opendal_core::OperationContext;
use opendal_core::Result;
use opendal_core::raw::*;

pub type TosListers = TwoWays<oio::PageLister<TosLister>, oio::PageLister<TosObjectVersionsLister>>;

pub struct TosLister {
    core: Arc<TosCore>,
    ctx: OperationContext,

    path: String,
    args: OpList,

    delimiter: &'static str,
    start_after: Option<String>,
}

impl TosLister {
    pub fn new(core: Arc<TosCore>, ctx: OperationContext, path: &str, args: OpList) -> Self {
        let delimiter = if args.recursive() { "" } else { "/" };
        let start_after = args.start_after().map(ToString::to_string);

        Self {
            core,
            ctx,

            path: path.to_string(),
            args,

            delimiter,
            start_after,
        }
    }
}

impl oio::PageList for TosLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self
            .core
            .tos_list_objects_v2(
                &self.ctx,
                &self.path,
                &ctx.token,
                self.delimiter,
                self.args.limit(),
                if ctx.token.is_empty() {
                    self.start_after.clone()
                } else {
                    None
                },
            )
            .await?;

        if resp.status() != http::StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListObjectsV2")),
                resp,
            ));
        }

        let bs = resp.into_body();
        let output: ListObjectsOutputV2 =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        ctx.done = !output.is_truncated;
        ctx.token = output.next_continuation_token.clone().unwrap_or_default();

        for prefix in output.common_prefixes {
            let de = oio::Entry::new(
                &build_rel_path(&self.core.root, &prefix.prefix),
                MetadataBuilder::dir().build(),
            );

            ctx.entries.push_back(de);
        }

        for object in output.contents {
            let mut path = build_rel_path(&self.core.root, &object.key);
            if path.is_empty() {
                path = "/".to_string();
            }

            let mut meta = if path.ends_with('/') {
                MetadataBuilder::dir()
            } else {
                MetadataBuilder::file(object.size)
            };
            meta.is_current(Some(true));
            if let Some(etag) = &object.etag {
                meta.etag(etag);
                meta.content_md5(etag.trim_matches('"'));
            }
            meta.last_modified(object.last_modified.parse::<Timestamp>()?);

            let de = oio::Entry::with(path, meta.build());
            ctx.entries.push_back(de);
        }

        Ok(())
    }
}

pub struct TosObjectVersionsLister {
    core: Arc<TosCore>,
    ctx: OperationContext,

    prefix: String,
    args: OpList,

    delimiter: &'static str,
    abs_start_after: Option<String>,
}

impl TosObjectVersionsLister {
    pub fn new(core: Arc<TosCore>, ctx: OperationContext, path: &str, args: OpList) -> Self {
        let delimiter = if args.recursive() { "" } else { "/" };
        let abs_start_after = args
            .start_after()
            .map(|start_after| build_abs_path(&core.root, start_after));

        Self {
            core,
            ctx,
            prefix: path.to_string(),
            args,
            delimiter,
            abs_start_after,
        }
    }
}

impl oio::PageList for TosObjectVersionsLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let markers = ctx.token.rsplit_once(" ");
        let (key_marker, version_id_marker) = if let Some(data) = markers {
            data
        } else if let Some(start_after) = &self.abs_start_after {
            (start_after.as_str(), "")
        } else {
            ("", "")
        };

        let resp = self
            .core
            .tos_list_object_versions(
                &self.ctx,
                &self.prefix,
                self.delimiter,
                self.args.limit(),
                key_marker,
                version_id_marker,
            )
            .await?;

        if resp.status() != http::StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListObjectVersions")),
                resp,
            ));
        }

        let bs = resp.into_body();
        let output: ListObjectVersionsOutput =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        ctx.done = if let Some(is_truncated) = output.is_truncated {
            !is_truncated
        } else {
            false
        };
        ctx.token = format!(
            "{} {}",
            output.next_key_marker.unwrap_or_default(),
            output.next_version_id_marker.unwrap_or_default()
        );

        for prefix in output.common_prefixes {
            let de = oio::Entry::new(
                &build_rel_path(&self.core.root, &prefix.prefix),
                MetadataBuilder::dir().build(),
            );

            ctx.entries.push_back(de);
        }

        for version_object in output.versions {
            if !(self.args.versions() || version_object.is_latest) {
                continue;
            }

            let mut path = build_rel_path(&self.core.root, &version_object.key);
            if path.is_empty() {
                path = "/".to_string();
            }

            let mut meta = if path.ends_with('/') {
                MetadataBuilder::dir()
            } else {
                MetadataBuilder::file(version_object.size)
            };
            meta.version(&version_object.version_id);
            meta.is_current(Some(version_object.is_latest));
            meta.last_modified(version_object.last_modified.parse::<Timestamp>()?);

            if let Some(etag) = version_object.etag {
                meta.etag(&etag);
                meta.content_md5(etag.trim_matches('"'));
            }

            let de = oio::Entry::with(path, meta.build());
            ctx.entries.push_back(de);
        }

        if self.args.deleted() {
            for delete_marker in output.delete_markers {
                let mut path = build_rel_path(&self.core.root, &delete_marker.key);
                if path.is_empty() {
                    path = "/".to_string();
                }

                let mut meta = if path.ends_with('/') {
                    MetadataBuilder::dir()
                } else {
                    MetadataBuilder::file(0)
                };
                meta.version(&delete_marker.version_id);
                meta.is_deleted(true);
                meta.is_current(Some(delete_marker.is_latest));
                meta.last_modified(delete_marker.last_modified.parse::<Timestamp>()?);

                let de = oio::Entry::with(path, meta.build());
                ctx.entries.push_back(de);
            }
        }

        Ok(())
    }
}
