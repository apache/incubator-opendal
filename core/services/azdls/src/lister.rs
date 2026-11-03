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
use serde::Deserialize;
use serde_json::de;

use super::core::parse_error;
use super::core::{AzdlsCore, ErrorContext};
use opendal_core::raw::*;
use opendal_core::*;

pub struct AzdlsLister {
    core: Arc<AzdlsCore>,
    ctx: OperationContext,

    path: String,
    limit: Option<usize>,
}

impl AzdlsLister {
    pub fn new(
        core: Arc<AzdlsCore>,
        ctx: OperationContext,
        path: String,
        limit: Option<usize>,
    ) -> Self {
        Self {
            core,
            ctx,
            path,
            limit,
        }
    }
}

impl oio::PageList for AzdlsLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let resp = self
            .core
            .azdls_list(&self.ctx, &self.path, &ctx.token, self.limit)
            .await?;

        // azdls will return not found for not-exist path.
        if resp.status() == http::StatusCode::NOT_FOUND {
            ctx.done = true;
            return Ok(());
        }

        if resp.status() != http::StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("ListPaths")),
                resp,
            ));
        }

        // Return self at the first page.
        if ctx.token.is_empty() && !ctx.done {
            let e = oio::Entry::new(&self.path, MetadataBuilder::dir().build());
            ctx.entries.push_back(e);
        }

        // Check whether this list is done.
        if let Some(value) = resp.headers().get("x-ms-continuation") {
            let value = value.to_str().map_err(|err| {
                Error::new(ErrorKind::Unexpected, "header value is not valid string")
                    .set_source(err)
            })?;
            ctx.token = value.to_string();
        } else {
            ctx.token = "".to_string();
            ctx.done = true;
        }

        let bs = resp.into_body();

        let output: Output = de::from_reader(bs.reader()).map_err(new_json_deserialize_error)?;

        for object in output.paths {
            // Azdls will return `"true"` and `"false"` for is_directory.
            let mode = if &object.is_directory == "true" {
                EntryMode::DIR
            } else {
                EntryMode::FILE
            };

            let content_length = object.content_length.parse().map_err(|err| {
                Error::new(ErrorKind::Unexpected, "content length is not valid integer")
                    .set_source(err)
            })?;
            let mut meta = if mode == EntryMode::FILE {
                MetadataBuilder::file(content_length)
            } else {
                MetadataBuilder::dir()
            };
            meta.etag(format!("\"{}\"", object.etag))
                .last_modified(Timestamp::parse_rfc2822(&object.last_modified)?);

            let mut path = build_rel_path(&self.core.root, &object.name);
            if mode.is_dir() {
                path += "/"
            };

            let de = oio::Entry::new(&path, meta.build());

            ctx.entries.push_back(de);
        }

        Ok(())
    }
}

/// # Examples
///
/// ```json
/// {"paths":[{"contentLength":"1977097","etag":"0x8DACF9B0061305F","group":"$superuser","lastModified":"Sat, 26 Nov 2022 10:43:05 GMT","name":"c3b3ef48-7783-4946-81bc-dc07e1728878/d4ea21d7-a533-4011-8b1f-d0e566d63725","owner":"$superuser","permissions":"rw-r-----"}]}
/// ```
#[derive(Default, Debug, Deserialize)]
#[serde(default)]
struct Output {
    paths: Vec<Path>,
}

#[derive(Default, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
struct Path {
    #[serde(rename = "contentLength")]
    content_length: String,
    #[serde(rename = "etag")]
    etag: String,
    /// Azdls will return `"true"` and `"false"` for is_directory.
    #[serde(rename = "isDirectory")]
    is_directory: String,
    #[serde(rename = "lastModified")]
    last_modified: String,
    #[serde(rename = "name")]
    name: String,
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_parse_path() {
        let bs = Bytes::from(
            r#"{"paths":[{"contentLength":"1977097","etag":"0x8DACF9B0061305F","group":"$superuser","lastModified":"Sat, 26 Nov 2022 10:43:05 GMT","name":"c3b3ef48-7783-4946-81bc-dc07e1728878/d4ea21d7-a533-4011-8b1f-d0e566d63725","owner":"$superuser","permissions":"rw-r-----"}]}"#,
        );
        let out: Output = de::from_slice(&bs).expect("must success");
        println!("{out:?}");

        assert_eq!(
            out.paths[0],
            Path {
                content_length: "1977097".to_string(),
                etag: "0x8DACF9B0061305F".to_string(),
                is_directory: "".to_string(),
                last_modified: "Sat, 26 Nov 2022 10:43:05 GMT".to_string(),
                name: "c3b3ef48-7783-4946-81bc-dc07e1728878/d4ea21d7-a533-4011-8b1f-d0e566d63725"
                    .to_string()
            }
        );
    }
}
