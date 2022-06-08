// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::future::Future;
use std::io::Result;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use anyhow::anyhow;
use bytes::Buf;
use bytes::BufMut;
use futures::future::BoxFuture;
use futures::ready;
use futures::StreamExt;
use log::debug;
use quick_xml::de;
use serde::Deserialize;

use super::Backend;
use crate::error::other;
use crate::error::ObjectError;
use crate::DirEntry;
use crate::ObjectMode;

pub struct DirStream {
    backend: Arc<Backend>,
    path: String,

    token: String,
    done: bool,
    state: State,
}

enum State {
    Idle,
    Sending(BoxFuture<'static, Result<bytes::Bytes>>),
    Listing((Output, usize, usize)),
}

impl DirStream {
    pub fn new(backend: Arc<Backend>, path: &str) -> Self {
        Self {
            backend,
            path: path.to_string(),

            token: "".to_string(),
            done: false,
            state: State::Idle,
        }
    }
}

impl futures::Stream for DirStream {
    type Item = Result<DirEntry>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let backend = self.backend.clone();

        match &mut self.state {
            State::Idle => {
                let path = self.path.clone();
                let token = self.token.clone();
                let fut = async move {
                    let mut resp = backend.list_objects(&path, &token).await?;

                    if resp.status() != http::StatusCode::OK {
                        let e = other(ObjectError::new("list", &path, anyhow!("{:?}", resp)));

                        debug!("error response: {:?}", resp);
                        return Err(e);
                    }

                    let body = resp.body_mut();
                    let mut bs = bytes::BytesMut::new();
                    while let Some(b) = body.next().await {
                        let b = b.map_err(|e| {
                            other(ObjectError::new(
                                "list",
                                &path,
                                anyhow!("read body: {:?}", e),
                            ))
                        })?;
                        bs.put_slice(&b)
                    }

                    Ok(bs.freeze())
                };
                self.state = State::Sending(Box::pin(fut));
                self.poll_next(cx)
            }
            State::Sending(fut) => {
                let bs = ready!(Pin::new(fut).poll(cx))?;
                let output: Output = de::from_reader(bs.reader()).map_err(|e| {
                    other(ObjectError::new(
                        "list",
                        &self.path,
                        anyhow!("deserialize list_bucket output: {:?}", e),
                    ))
                })?;

                // Try our best to check whether this list is done.
                //
                // - Check `is_truncated`
                // - Check `next_continuation_token`
                // - Check the length of `common_prefixes` and `contents` (very rarely case)
                self.done = if let Some(is_truncated) = output.is_truncated {
                    !is_truncated
                } else if let Some(next_continuation_token) =
                    output.next_continuation_token.as_ref()
                {
                    next_continuation_token.is_empty()
                } else {
                    output.common_prefixes.is_empty() && output.contents.is_empty()
                };
                self.token = output.next_continuation_token.clone().unwrap_or_default();
                self.state = State::Listing((output, 0, 0));
                self.poll_next(cx)
            }
            State::Listing((output, common_prefixes_idx, objects_idx)) => {
                let prefixes = &output.common_prefixes;
                if *common_prefixes_idx < prefixes.len() {
                    *common_prefixes_idx += 1;
                    let prefix = &prefixes[*common_prefixes_idx - 1].prefix;

                    let de = DirEntry::new(
                        backend.clone(),
                        ObjectMode::DIR,
                        &backend.get_rel_path(prefix),
                    );

                    debug!(
                        "object {} got entry, mode: {}, path: {}",
                        &self.path,
                        de.path(),
                        de.mode()
                    );
                    return Poll::Ready(Some(Ok(de)));
                }

                let objects = &output.contents;
                while *objects_idx < objects.len() {
                    let object = &objects[*objects_idx];
                    *objects_idx += 1;

                    // s3 could return the dir itself in contents
                    // which endswith `/`.
                    // We should ignore them.
                    if object.key.ends_with('/') {
                        continue;
                    }

                    let de = DirEntry::new(
                        backend.clone(),
                        ObjectMode::FILE,
                        &backend.get_rel_path(&object.key),
                    );

                    debug!(
                        "dir object {} got entry, mode: {}, path: {}",
                        &self.path,
                        de.mode(),
                        de.path()
                    );
                    return Poll::Ready(Some(Ok(de)));
                }

                if self.done {
                    debug!("object {} list done", &self.path);
                    return Poll::Ready(None);
                }

                self.state = State::Idle;
                self.poll_next(cx)
            }
        }
    }
}

/// Output of ListBucket/ListObjects.
///
/// ## Note
///
/// Use `Option` in `is_truncated` and `next_continuation_token` to make
/// the behavior more clear so that we can be compatible to more s3 services.
///
/// And enable `serde(default)` so that we can keep going even when some field
/// is not exist.
#[derive(Default, Debug, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct Output {
    is_truncated: Option<bool>,
    next_continuation_token: Option<String>,
    common_prefixes: Vec<OutputCommonPrefix>,
    contents: Vec<OutputContent>,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct OutputContent {
    key: String,
    size: u64,
}

#[derive(Default, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct OutputCommonPrefix {
    prefix: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list_output() {
        let bs = bytes::Bytes::from(
            r#"<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>example-bucket</Name>
  <Prefix>photos/2006/</Prefix>
  <KeyCount>3</KeyCount>
  <MaxKeys>1000</MaxKeys>
  <Delimiter>/</Delimiter>
  <IsTruncated>false</IsTruncated>
  <Contents>
    <Key>photos/2006</Key>
    <LastModified>2016-04-30T23:51:29.000Z</LastModified>
    <ETag>"d41d8cd98f00b204e9800998ecf8427e"</ETag>
    <Size>56</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>
  <Contents>
    <Key>photos/2007</Key>
    <LastModified>2016-04-30T23:51:29.000Z</LastModified>
    <ETag>"d41d8cd98f00b204e9800998ecf8427e"</ETag>
    <Size>100</Size>
    <StorageClass>STANDARD</StorageClass>
  </Contents>

  <CommonPrefixes>
    <Prefix>photos/2006/February/</Prefix>
  </CommonPrefixes>
  <CommonPrefixes>
    <Prefix>photos/2006/January/</Prefix>
  </CommonPrefixes>
</ListBucketResult>"#,
        );

        let out: Output = de::from_reader(bs.reader()).expect("must success");

        println!("{:?}", out);
        assert!(!out.is_truncated.unwrap());
        assert!(out.next_continuation_token.is_none());
        assert_eq!(
            out.common_prefixes
                .iter()
                .map(|v| v.prefix.clone())
                .collect::<Vec<String>>(),
            vec!["photos/2006/February/", "photos/2006/January/"]
        );
        assert_eq!(
            out.contents,
            vec![
                OutputContent {
                    key: "photos/2006".to_string(),
                    size: 56
                },
                OutputContent {
                    key: "photos/2007".to_string(),
                    size: 100
                }
            ]
        )
    }
}
