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

use super::core::{ErrorContext, HfCore, PathInfo, parse_error};
use opendal_core::raw::*;
use opendal_core::*;

struct FileTree {
    files: Vec<PathInfo>,
    next_cursor: Option<String>,
}

/// Extract the cursor value from a Link header's "next" URL.
fn parse_next_cursor(link_str: &str) -> Option<String> {
    for link in link_str.split(',') {
        if link.contains("rel=\"next\"") || link.contains("rel='next'") {
            let (_, rest) = link.split_once('<')?;
            let (url, _) = rest.split_once('>')?;
            let query = url.split_once('?')?.1;
            return query
                .split('&')
                .find_map(|p| p.strip_prefix("cursor="))
                .map(|v| v.to_string());
        }
    }
    None
}

pub struct HfLister {
    core: Arc<HfCore>,
    ctx: OperationContext,
    /// The directory path to list via the tree API (always ends with `/` or is empty for root).
    list_path: String,
    /// When the original path didn't end with `/`, filter results to this prefix.
    prefix: Option<String>,
    recursive: bool,
}

impl HfLister {
    pub fn new(core: Arc<HfCore>, ctx: OperationContext, path: String, recursive: bool) -> Self {
        if path.is_empty() || path.ends_with('/') {
            Self {
                core,
                ctx,
                list_path: path,
                prefix: None,
                recursive,
            }
        } else {
            // Prefix listing: list the parent directory and filter by prefix.
            let parent = match path.rfind('/') {
                Some(pos) => path[..=pos].to_string(),
                None => String::new(),
            };
            Self {
                core,
                ctx,
                list_path: parent,
                prefix: Some(path),
                recursive,
            }
        }
    }

    async fn file_tree(
        &self,
        path: &str,
        recursive: bool,
        cursor: Option<&str>,
    ) -> Result<FileTree> {
        let uri = self.core.repo.uri(&self.core.root, path);
        let url = uri.file_tree_url(&self.core.endpoint, recursive, cursor);

        let req = self
            .core
            .request(http::Method::GET, &url, Operation::List, "FileTree")?
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        let resp = self.core.send(&self.ctx, req).await?;
        if !resp.status().is_success() {
            let (parts, _) = resp.into_parts();
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("FileTree")),
                parts,
            ));
        }
        let (parts, mut body) = resp.into_parts();
        let buffer = body.to_buffer().await?;
        let files: Vec<PathInfo> =
            serde_json::from_reader(buffer.reader()).map_err(new_json_deserialize_error)?;

        let next_cursor = parts
            .headers
            .get(http::header::LINK)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_next_cursor);

        Ok(FileTree { files, next_cursor })
    }
}

impl oio::PageList for HfLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let cursor = if ctx.token.is_empty() {
            None
        } else {
            Some(ctx.token.as_str())
        };

        let response = match self
            .file_tree(&self.list_path, self.recursive, cursor)
            .await
        {
            Ok(r) => r,
            // HF returns 404 when a path doesn't exist; treat as empty listing.
            Err(e) if e.kind() == ErrorKind::NotFound => {
                ctx.done = true;
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        if let Some(next_cursor) = response.next_cursor {
            ctx.token = next_cursor;
        } else {
            ctx.done = true;
        }

        for info in response.files {
            let meta = info.metadata()?;
            let path = if meta.mode() == EntryMode::DIR {
                format!("{}/", info.path)
            } else {
                info.path.clone()
            };
            let rel_path = build_rel_path(&self.core.root, &path);

            // Filter by prefix when doing prefix-based listing.
            if let Some(prefix) = &self.prefix
                && !rel_path.starts_with(prefix.as_str())
            {
                continue;
            }

            ctx.entries.push_back(oio::Entry::new(&rel_path, meta));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use super::super::core::HfRepoType;
    use super::super::core::test_utils::create_test_core;
    use super::*;
    use opendal_core::raw::oio::PageList;

    #[test]
    fn test_parse_next_cursor() {
        let link =
            r#"<https://huggingface.co/api/models/org/model/tree/main?cursor=abc123>; rel="next""#;
        assert_eq!(parse_next_cursor(link), Some("abc123".to_string()));
    }

    #[test]
    fn test_parse_next_cursor_no_next() {
        let link =
            r#"<https://huggingface.co/api/models/org/model/tree/main?cursor=abc123>; rel="prev""#;
        assert_eq!(parse_next_cursor(link), None);
    }

    #[tokio::test]
    async fn test_list_model_root() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        let lister = HfLister::new(Arc::new(core), ctx, String::new(), false);
        let mut page_ctx = oio::PageContext {
            done: false,
            token: String::new(),
            entries: VecDeque::new(),
        };

        lister.next_page(&mut page_ctx).await?;

        assert_eq!(
            mock_client.get_captured_url(),
            "https://huggingface.co/api/models/test-user/test-repo/tree/main?expand=True"
        );
        assert!(page_ctx.done);
        let entry = page_ctx.entries.pop_front().expect("entry must exist");
        assert_eq!(entry.path(), "test.txt");
        assert_eq!(entry.metadata().mode(), EntryMode::FILE);
        assert_eq!(entry.metadata().content_length(), 100);

        Ok(())
    }
}
