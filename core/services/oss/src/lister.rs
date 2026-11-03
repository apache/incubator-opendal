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

use super::core::parse_error;
use super::core::*;
use opendal_core::raw::oio::PageContext;
use opendal_core::raw::*;
use opendal_core::*;

pub type OssListers = TwoWays<oio::PageLister<OssLister>, oio::PageLister<OssObjectVersionsLister>>;

pub struct OssLister {
    core: Arc<OssCore>,
    ctx: OperationContext,

    path: String,
    delimiter: &'static str,
    limit: Option<usize>,
    /// Filter results to objects whose names are lexicographically
    /// **equal to or after** startOffset
    start_after: Option<String>,
}

impl OssLister {
    pub fn new(
        core: Arc<OssCore>,
        ctx: OperationContext,
        path: &str,
        recursive: bool,
        limit: Option<usize>,
        start_after: Option<&str>,
    ) -> Self {
        let delimiter = if recursive { "" } else { "/" };
        Self {
            core,
            ctx,
            path: path.to_string(),
            delimiter,
            limit,
            start_after: start_after.map(String::from),
        }
    }
}

impl oio::PageList for OssLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self
            .core
            .oss_list_object(
                &self.ctx,
                &self.path,
                &ctx.token,
                self.delimiter,
                self.limit,
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

        let output: ListObjectsOutput = de::from_reader(bs.reader())
            .map_err(|e| Error::new(ErrorKind::Unexpected, "deserialize xml").set_source(e))?;

        ctx.done = !output.is_truncated;
        ctx.token = output.next_continuation_token.unwrap_or_default();

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
            if self.start_after.as_ref() == Some(&path) {
                continue;
            }

            let mut meta = if path.ends_with('/') {
                MetadataBuilder::dir()
            } else {
                MetadataBuilder::file(object.size)
            };
            meta.is_current(Some(true));
            meta.etag(&object.etag);
            meta.content_md5(object.etag.trim_matches('"'));
            meta.last_modified(object.last_modified.parse::<Timestamp>()?);

            let de = oio::Entry::with(path, meta.build());
            ctx.entries.push_back(de);
        }

        Ok(())
    }
}

/// refer: https://help.aliyun.com/zh/oss/developer-reference/listobjectversions?spm=a2c4g.11186623.help-menu-31815.d_3_1_1_5_5_2.53f67237GJlMPw&scm=20140722.H_112467._.OR_help-T_cn~zh-V_1
pub struct OssObjectVersionsLister {
    core: Arc<OssCore>,
    ctx: OperationContext,

    prefix: String,
    args: OpList,

    delimiter: &'static str,
    abs_start_after: Option<String>,
}

impl OssObjectVersionsLister {
    pub fn new(core: Arc<OssCore>, ctx: OperationContext, path: &str, args: OpList) -> Self {
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

impl oio::PageList for OssObjectVersionsLister {
    async fn next_page(&self, ctx: &mut PageContext) -> Result<()> {
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
            .oss_list_object_versions(
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

        let body = resp.into_body();
        let output: ListObjectVersionsOutput = de::from_reader(body.reader())
            .map_err(new_xml_deserialize_error)
            // Allow Cos list to retry on XML deserialization errors.
            //
            // This is because the Cos list API may return incomplete XML data under high load.
            // We are confident that our XML decoding logic is correct. When this error occurs,
            // we allow retries to obtain the correct data.
            .map_err(Error::set_temporary)?;

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

        for version_object in output.version {
            // `list` must be additive, so we need to include the latest version object
            // even if `versions` is not enabled.
            //
            // Here we skip all non-latest version objects if `versions` is not enabled.
            if !(self.args.versions() || version_object.is_latest) {
                continue;
            }

            let mut path = build_rel_path(&self.core.root, &version_object.key);
            if path.is_empty() {
                path = "/".to_owned();
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

            let entry = oio::Entry::new(&path, meta.build());
            ctx.entries.push_back(entry);
        }

        if self.args.deleted() {
            for delete_marker in output.delete_marker {
                let mut path = build_rel_path(&self.core.root, &delete_marker.key);
                if path.is_empty() {
                    path = "/".to_owned();
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

                let entry = oio::Entry::new(&path, meta.build());
                ctx.entries.push_back(entry);
            }
        }

        Ok(())
    }
}
