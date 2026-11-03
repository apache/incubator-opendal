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
use opendal_core::raw::*;
use opendal_core::*;

use super::core::parse_error;
use super::core::{CloudflareKvCore, ErrorContext};
use super::model::{CfKvListKey, CfKvListResponse};

pub struct CloudflareKvLister {
    core: Arc<CloudflareKvCore>,
    ctx: OperationContext,

    path: String,
    limit: Option<usize>,
    recursive: bool,
}

/// Strip the service root from a root-prefixed key.
///
/// The keys returned by the KV API carry the service root as a prefix, so only that prefix may be
/// removed. `str::replace` removes *every* occurrence anywhere in the key, which silently mangles
/// any key that repeats the root as an inner path segment.
///
/// The sibling object-store listers reach for `build_rel_path` here, but that helper
/// `debug_assert!`s that the path really does start with the root -- a guarantee this service does
/// not enforce on the values the API hands back -- so this stays total and leaves a
/// non-root-prefixed key untouched.
fn relative_to_root(root: &str, name: &str) -> String {
    name.strip_prefix(root.trim_start_matches('/'))
        .unwrap_or(name)
        .to_string()
}

impl CloudflareKvLister {
    pub fn new(
        core: Arc<CloudflareKvCore>,
        ctx: OperationContext,
        path: &str,
        recursive: bool,
        limit: Option<usize>,
    ) -> Self {
        Self {
            core,
            ctx,

            path: path.to_string(),
            limit,
            recursive,
        }
    }

    fn build_entry_for_item(&self, item: &CfKvListKey, root: &str) -> Result<oio::Entry> {
        let metadata = item.metadata.clone();
        let mut name = item.name.clone();

        if metadata.is_dir && !name.ends_with('/') {
            name += "/";
        }

        let mut name = relative_to_root(root, &name);

        // If it is the root directory, it needs to be processed as /
        if name.is_empty() {
            name = "/".to_string();
        }

        let entry_metadata = if name.ends_with('/') {
            let mut metadata = MetadataBuilder::dir();
            metadata.etag(build_tmp_path_of(&name));
            metadata.build()
        } else {
            let mut result = MetadataBuilder::file(metadata.content_length as u64);
            result
                .etag(&metadata.etag)
                .last_modified(metadata.last_modified.parse::<Timestamp>()?);
            result.build()
        };

        Ok(oio::Entry::new(&name, entry_metadata))
    }

    fn handle_non_recursive_file_list(
        &self,
        ctx: &mut oio::PageContext,
        result: &[CfKvListKey],
        root: &str,
    ) -> Result<()> {
        if let Some(item) = result.iter().find(|item| item.name == self.path) {
            let entry = self.build_entry_for_item(item, root)?;
            ctx.entries.push_back(entry);
        } else if !result.is_empty() {
            let path_name = relative_to_root(root, &self.path);
            let entry = oio::Entry::new(&format!("{path_name}/"), {
                let mut metadata = MetadataBuilder::dir();
                metadata.etag(build_tmp_path_of(&path_name));
                metadata.build()
            });
            ctx.entries.push_back(entry);
        }
        ctx.done = true;
        Ok(())
    }
}

impl oio::PageList for CloudflareKvLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let new_path = self.path.trim_end_matches('/');
        let resp = self
            .core
            .list(&self.ctx, new_path, self.limit, Some(ctx.token.clone()))
            .await?;

        if resp.status() != http::StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListKeys")),
                resp,
            ));
        }

        let bs = resp.into_body();
        let res: CfKvListResponse =
            serde_json::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        if !res.success {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "oss list this key failed for reason we don't know",
            ));
        }

        let (token, done) = res
            .result_info
            .and_then(|info| info.cursor)
            .map_or((String::new(), true), |cursor| {
                (cursor.clone(), cursor.is_empty())
            });

        ctx.token = token;
        ctx.done = done;

        if let Some(result) = res.result {
            let root = self.core.info.root().to_string();

            if !self.path.ends_with('/') && !self.recursive {
                self.handle_non_recursive_file_list(ctx, &result, &root)?;
                return Ok(());
            }

            for item in result {
                let mut name = item.name.clone();
                if item.metadata.is_dir && !name.ends_with('/') {
                    name += "/";
                }

                // For non-recursive listing, filter out entries not in the current directory.
                if !self.recursive {
                    if let Some(relative_path) = name.strip_prefix(&self.path) {
                        if relative_path.trim_end_matches('/').contains('/') {
                            continue;
                        }
                    } else if self.path != name {
                        continue;
                    }
                }

                let entry = self.build_entry_for_item(&item, &root)?;
                ctx.entries.push_back(entry);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_to_root_strips_only_the_prefix() {
        // The root repeated as an inner segment must survive; only the leading copy goes.
        assert_eq!(
            relative_to_root("/data/", "data/backup/data/file.txt"),
            "backup/data/file.txt"
        );
        assert_eq!(relative_to_root("/data/", "data/file.txt"), "file.txt");
    }

    #[test]
    fn relative_to_root_does_not_match_mid_key() {
        // "replace" would turn this into "xfile.txt" by deleting a substring that is not a prefix.
        assert_eq!(
            relative_to_root("/data/", "xdata/file.txt"),
            "xdata/file.txt"
        );
    }

    #[test]
    fn relative_to_root_handles_the_root_itself_and_a_bare_root() {
        assert_eq!(relative_to_root("/data/", "data/"), "");
        assert_eq!(relative_to_root("/", "file.txt"), "file.txt");
    }
}
