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

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::mem;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;

use anyhow::anyhow;
use async_trait::async_trait;
use http::header::CONTENT_LENGTH;
use http::StatusCode;
use isahc::AsyncBody;
use isahc::AsyncReadResponseExt;
use log::debug;
use log::info;
use radix_trie::Trie;
use radix_trie::TrieCommon;

use super::error::parse_error;
use crate::error::other;
use crate::error::BackendError;
use crate::error::ObjectError;
use crate::http_util::new_request_build_error;
use crate::http_util::new_request_send_error;
use crate::http_util::new_response_consume_error;
use crate::http_util::parse_content_length;
use crate::http_util::parse_content_md5;
use crate::http_util::parse_error_response;
use crate::http_util::parse_etag;
use crate::http_util::parse_last_modified;
use crate::http_util::percent_encode_path;
use crate::http_util::HttpClient;
use crate::io_util::unshared_reader;
use crate::ops::OpCreate;
use crate::ops::OpDelete;
use crate::ops::OpList;
use crate::ops::OpRead;
use crate::ops::OpStat;
use crate::ops::OpWrite;
use crate::ops::{BytesRange, Operation};
use crate::Accessor;
use crate::AccessorMetadata;
use crate::BytesReader;
use crate::DirEntry;
use crate::DirStreamer;
use crate::ObjectMetadata;
use crate::ObjectMode;
use crate::Scheme;

/// Builder for http backend.
#[derive(Default)]
pub struct Builder {
    endpoint: Option<String>,
    root: Option<String>,
    index: Trie<String, ()>,
}

impl Debug for Builder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut de = f.debug_struct("Builder");
        de.field("endpoint", &self.endpoint);
        de.field("root", &self.root);
        de.field("index", &format!("length: {}", self.index.len()));

        de.finish()
    }
}

impl Builder {
    /// Set endpoint for http backend.
    ///
    /// For example: `https://example.com`
    pub fn endpoint(&mut self, endpoint: &str) -> &mut Self {
        self.endpoint = if endpoint.is_empty() {
            None
        } else {
            Some(endpoint.to_string())
        };

        self
    }

    /// Set root path of http backend.
    pub fn root(&mut self, root: &str) -> &mut Self {
        self.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    pub(crate) fn insert_path(&mut self, path: &str) {
        for (idx, _) in path.match_indices('/') {
            let p = path[..=idx].to_string();
            if self.index.get(&p).is_none() {
                debug!("insert path {} into index", p);
                self.index.insert(p, ());
            }
        }
        if self.index.get(path).is_none() {
            debug!("insert path {} into index", path);
            self.index.insert(path.to_string(), ());
        }
    }

    /// Insert index into backend.
    pub fn insert_index(&mut self, key: &str) -> &mut Self {
        if key.is_empty() {
            return self;
        }

        let key = if let Some(stripped) = key.strip_prefix('/') {
            stripped.to_string()
        } else {
            key.to_string()
        };

        self.insert_path(&key);

        self
    }

    /// Extend index from an iterator.
    pub fn extend_index<'a>(&mut self, it: impl Iterator<Item = &'a str>) -> &mut Self {
        for k in it.filter(|v| !v.is_empty()) {
            let k = if let Some(stripped) = k.strip_prefix('/') {
                stripped.to_string()
            } else {
                k.to_string()
            };

            self.insert_path(&k);
        }
        self
    }

    /// Build a HTTP backend.
    pub fn build(&mut self) -> Result<Backend> {
        info!("backend build started: {:?}", &self);

        let endpoint = match &self.endpoint {
            None => {
                return Err(other(BackendError::new(
                    HashMap::new(),
                    anyhow!("endpoint must be specified"),
                )))
            }
            Some(v) => v,
        };

        // Make `/` as the default of root.
        let root = match &self.root {
            None => "/".to_string(),
            Some(v) => {
                debug_assert!(!v.is_empty());

                let mut v = v.clone();
                if !v.starts_with('/') {
                    return Err(other(BackendError::new(
                        HashMap::from([("root".to_string(), v.clone())]),
                        anyhow!("root must start with /"),
                    )));
                }
                if !v.ends_with('/') {
                    v.push('/');
                }

                v
            }
        };

        let client = HttpClient::new();

        info!("backend build finished: {:?}", &self);
        Ok(Backend {
            endpoint: endpoint.to_string(),
            root,
            client,
            index: Arc::new(Mutex::new(mem::take(&mut self.index))),
        })
    }

    /// Build a HTTP backend.
    #[deprecated = "Use Builder::build() instead"]
    pub async fn finish(&mut self) -> Result<Arc<dyn Accessor>> {
        Ok(Arc::new(self.build()?))
    }
}

/// Backend is used to serve `Accessor` support for http.
#[derive(Clone)]
pub struct Backend {
    endpoint: String,
    root: String,
    client: HttpClient,
    index: Arc<Mutex<Trie<String, ()>>>,
}

impl Debug for Backend {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backend")
            .field("endpoint", &self.endpoint)
            .field("root", &self.root)
            .field("client", &self.client)
            .field(
                "index",
                &format!(
                    "length = {}",
                    self.index.lock().expect("lock must succeed").len()
                ),
            )
            .finish()
    }
}

impl Backend {
    /// Create a new builder for s3.
    #[deprecated = "Use Builder::default() instead"]
    pub fn build() -> Builder {
        Builder::default()
    }

    pub(crate) fn from_iter(it: impl Iterator<Item = (String, String)>) -> Result<Self> {
        let mut builder = Builder::default();

        for (k, v) in it {
            let v = v.as_str();
            match k.as_ref() {
                "root" => builder.root(v),
                "endpoint" => builder.endpoint(v),
                _ => continue,
            };
        }

        builder.build()
    }

    pub(crate) fn get_abs_path(&self, path: &str) -> String {
        if path == "/" {
            return self.root.to_string();
        }

        // root must be normalized like `/abc/`
        format!("{}{}", self.root, path)
    }

    pub(crate) fn get_index_path(&self, path: &str) -> String {
        match path.strip_prefix('/') {
            Some(strip) => strip.to_string(),
            None => path.to_string(),
        }
    }

    pub(crate) fn insert_path(&self, path: &str) {
        let mut index = self.index.lock().expect("lock must succeed");

        for (idx, _) in path.match_indices('/') {
            let p = path[..=idx].to_string();

            if index.get(&p).is_none() {
                debug!("insert path {} into index", p);
                index.insert(p, ());
            }
        }
        if index.get(path).is_none() {
            debug!("insert path {} into index", path);
            index.insert(path.to_string(), ());
        }
    }
}

#[async_trait]
impl Accessor for Backend {
    fn metadata(&self) -> AccessorMetadata {
        let mut ma = AccessorMetadata::default();
        ma.set_scheme(Scheme::Http)
            .set_root(&self.root)
            .set_capabilities(None);

        ma
    }

    async fn create(&self, args: &OpCreate) -> Result<()> {
        let p = self.get_abs_path(args.path());

        let req = self.http_put(&p, AsyncBody::from_bytes_static("")).await?;
        let resp = self
            .client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Create, args.path(), e))?;

        match resp.status() {
            StatusCode::CREATED | StatusCode::OK => {
                self.insert_path(&self.get_index_path(args.path()));
                Ok(())
            }
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Create, args.path(), er);
                Err(err)
            }
        }
    }

    async fn read(&self, args: &OpRead) -> Result<BytesReader> {
        let p = self.get_abs_path(args.path());

        let resp = self.http_get(&p, args.offset(), args.size()).await?;

        match resp.status() {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => Ok(Box::new(resp.into_body())),
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Read, args.path(), er);
                Err(err)
            }
        }
    }

    async fn write(&self, args: &OpWrite, r: BytesReader) -> Result<u64> {
        let p = self.get_abs_path(args.path());

        let req = self
            .http_put(
                &p,
                AsyncBody::from_reader_sized(unshared_reader(r), args.size()),
            )
            .await?;

        let mut resp = self
            .client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Write, args.path(), e))?;

        match resp.status() {
            StatusCode::CREATED | StatusCode::OK => {
                self.insert_path(&self.get_index_path(args.path()));
                resp.consume()
                    .await
                    .map_err(|err| new_response_consume_error(Operation::Write, &p, err))?;
                Ok(args.size())
            }
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Write, args.path(), er);
                Err(err)
            }
        }
    }

    async fn stat(&self, args: &OpStat) -> Result<ObjectMetadata> {
        let p = self.get_abs_path(args.path());

        // Stat root always returns a DIR.
        if p == self.root {
            let mut m = ObjectMetadata::default();
            m.set_mode(ObjectMode::DIR);

            return Ok(m);
        }

        let resp = self.http_head(&p).await?;

        match resp.status() {
            StatusCode::OK => {
                let mut m = ObjectMetadata::default();

                if let Some(v) = parse_content_length(resp.headers())
                    .map_err(|e| other(ObjectError::new(Operation::Stat, &p, e)))?
                {
                    m.set_content_length(v);
                }

                if let Some(v) = parse_content_md5(resp.headers())
                    .map_err(|e| other(ObjectError::new(Operation::Stat, &p, e)))?
                {
                    m.set_content_md5(v);
                }

                if let Some(v) = parse_etag(resp.headers())
                    .map_err(|e| other(ObjectError::new(Operation::Stat, &p, e)))?
                {
                    m.set_etag(v);
                }

                if let Some(v) = parse_last_modified(resp.headers())
                    .map_err(|e| other(ObjectError::new(Operation::Stat, &p, e)))?
                {
                    m.set_last_modified(v);
                }

                if p.ends_with('/') {
                    m.set_mode(ObjectMode::DIR);
                } else {
                    m.set_mode(ObjectMode::FILE);
                };

                Ok(m)
            }
            StatusCode::NOT_FOUND if p.ends_with('/') => {
                let mut m = ObjectMetadata::default();
                m.set_mode(ObjectMode::DIR);

                Ok(m)
            }
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Stat, args.path(), er);
                Err(err)
            }
        }
    }

    async fn delete(&self, args: &OpDelete) -> Result<()> {
        let p = self.get_abs_path(args.path());

        let resp = self.http_delete(&p).await?;

        match resp.status() {
            StatusCode::NO_CONTENT | StatusCode::NOT_FOUND => {
                self.index.lock().expect("lock succeed").remove(args.path());
                Ok(())
            }
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Delete, args.path(), er);
                Err(err)
            }
        }
    }

    async fn list(&self, args: &OpList) -> Result<DirStreamer> {
        let mut path = args.path();
        if path == "/" {
            path = ""
        }

        let paths = match self.index.lock().expect("lock succeed").subtrie(path) {
            None => {
                return Err(Error::new(
                    ErrorKind::NotFound,
                    ObjectError::new(Operation::List, path, anyhow!("no such dir")),
                ))
            }
            Some(trie) => trie
                .keys()
                .filter_map(|k| {
                    let k = k.as_str();

                    // `/xyz` should not belong to `/abc`
                    if !k.starts_with(&path) {
                        return None;
                    }

                    // We should remove `/abc` if self
                    if k == path {
                        return None;
                    }

                    match k[path.len()..].find('/') {
                        // File `/abc/def.csv` must belong to `/abc`
                        None => Some(k.to_string()),
                        Some(idx) => {
                            // The index of first `/` after `/abc`.
                            let dir_idx = idx + 1 + path.len();

                            if dir_idx == k.len() {
                                // Dir `/abc/def/` belongs to `/abc/`
                                Some(k.to_string())
                            } else {
                                None
                            }
                        }
                    }
                })
                .collect::<HashSet<_>>(),
        };

        Ok(Box::new(DirStream {
            backend: Arc::new(self.clone()),
            paths: paths.into_iter().collect(),
            idx: 0,
        }))
    }
}

impl Backend {
    pub(crate) async fn http_get(
        &self,
        path: &str,
        offset: Option<u64>,
        size: Option<u64>,
    ) -> Result<isahc::Response<AsyncBody>> {
        let url = format!("{}{}", self.endpoint, percent_encode_path(path));

        let mut req = isahc::Request::get(&url);

        if offset.is_some() || size.is_some() {
            req = req.header(
                http::header::RANGE,
                BytesRange::new(offset, size).to_string(),
            );
        }

        let req = req
            .body(AsyncBody::empty())
            .map_err(|e| new_request_build_error(Operation::Read, path, e))?;

        self.client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Read, path, e))
    }

    pub(crate) async fn http_head(&self, path: &str) -> Result<isahc::Response<AsyncBody>> {
        let url = format!("{}{}", self.endpoint, percent_encode_path(path));

        let req = isahc::Request::head(&url);

        let req = req
            .body(AsyncBody::empty())
            .map_err(|e| new_request_build_error(Operation::Stat, path, e))?;

        self.client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Stat, path, e))
    }

    pub(crate) async fn http_put(
        &self,
        path: &str,
        body: AsyncBody,
    ) -> Result<isahc::Request<AsyncBody>> {
        let url = format!("{}/{}", self.endpoint, percent_encode_path(path));

        let mut req = isahc::Request::put(&url);

        if let Some(content_length) = body.len() {
            req = req.header(CONTENT_LENGTH, content_length)
        }

        // Set body
        let req = req
            .body(body)
            .map_err(|e| new_request_build_error(Operation::Write, path, e))?;

        Ok(req)
    }

    pub(crate) async fn http_delete(&self, path: &str) -> Result<isahc::Response<AsyncBody>> {
        let url = format!("{}/{}", self.endpoint, percent_encode_path(path));

        let req = isahc::Request::delete(&url);

        // Set body
        let req = req
            .body(AsyncBody::empty())
            .map_err(|e| new_request_build_error(Operation::Delete, path, e))?;

        self.client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Delete, path, e))
    }
}

struct DirStream {
    backend: Arc<Backend>,
    paths: Vec<String>,
    idx: usize,
}

impl futures::Stream for DirStream {
    type Item = Result<DirEntry>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.idx >= self.paths.len() {
            return Poll::Ready(None);
        }

        let idx = self.idx;
        self.idx += 1;

        let path = self.paths.get(idx).expect("path must valid");

        let de = if path.ends_with('/') {
            DirEntry::new(self.backend.clone(), ObjectMode::DIR, path)
        } else {
            DirEntry::new(self.backend.clone(), ObjectMode::FILE, path)
        };

        Poll::Ready(Some(Ok(de)))
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use futures::TryStreamExt;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;

    use super::*;
    use crate::Operator;

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let _ = env_logger::builder().is_test(true).try_init();

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/hello"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Hello, World!"))
            .mount(&mock_server)
            .await;

        let mut builder = Builder::default();
        builder.endpoint(&mock_server.uri());
        builder.root("/");
        builder.insert_index("/hello");
        let op = Operator::new(builder.build()?);

        let bs = op.object("hello").read().await?;

        assert_eq!(bs, b"Hello, World!");
        Ok(())
    }

    #[tokio::test]
    async fn test_stat() -> Result<()> {
        let _ = env_logger::builder().is_test(true).try_init();

        let mock_server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/hello"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-length", "128"))
            .mount(&mock_server)
            .await;

        let mut builder = Builder::default();
        builder.endpoint(&mock_server.uri());
        builder.root("/");
        builder.insert_index("/hello");
        let op = Operator::new(builder.build()?);

        let bs = op.object("hello").metadata().await?;

        assert_eq!(bs.mode(), ObjectMode::FILE);
        assert_eq!(bs.content_length(), 128);
        Ok(())
    }

    #[tokio::test]
    async fn test_list() -> Result<()> {
        let _ = env_logger::builder().is_test(true).try_init();

        let mock_server = MockServer::start().await;

        let mut expected = vec!["another/", "hello", "world"];

        let mut builder = Builder::default();
        builder.endpoint(&mock_server.uri());
        builder.root("/");
        for s in expected.iter() {
            builder.insert_index(s);
        }

        let op = Operator::new(builder.build()?);

        let bs = op.object("/").list().await?;
        let paths = bs.try_collect::<Vec<_>>().await?;
        let mut paths = paths
            .into_iter()
            .map(|v| v.path().to_string())
            .collect::<Vec<_>>();

        paths.sort_unstable();
        expected.sort_unstable();
        assert_eq!(paths, expected);
        Ok(())
    }
}
