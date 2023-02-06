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
use core::fmt::Debug;
use std::collections::HashMap;

use async_trait::async_trait;
use bytes::Buf;
use http::header::CONTENT_LENGTH;
use http::header::CONTENT_TYPE;
use http::response::Parts;
use http::Request;
use http::Response;
use http::StatusCode;
use log::debug;
use log::warn;
use serde::Deserialize;

use super::auth::WebHdfsAuth;
use super::dir_stream::DirStream;
use super::error::blocking_parse_error;
use super::error::blocking_resp_to_str;
use super::error::parse_error;
use super::percent_encode_path;
use crate::raw::build_abs_path;
use crate::raw::input;
use crate::raw::new_request_build_error;
use crate::raw::normalize_root;
use crate::raw::parse_into_object_metadata;
use crate::raw::Accessor;
use crate::raw::AccessorCapability;
use crate::raw::AccessorMetadata;
use crate::raw::AsyncBody;
use crate::raw::Body;
use crate::raw::BytesRange;
use crate::raw::HttpClient;
use crate::raw::IncomingAsyncBody;
use crate::raw::ObjectPage;
use crate::raw::ObjectPager;
use crate::raw::RpCreate;
use crate::raw::RpDelete;
use crate::raw::RpList;
use crate::raw::RpRead;
use crate::raw::RpStat;
use crate::raw::RpWrite;
use crate::Builder;
use crate::Error;
use crate::ErrorKind;
use crate::ObjectMetadata;
use crate::ObjectMode;
use crate::OpCreate;
use crate::OpDelete;
use crate::OpList;
use crate::OpRead;
use crate::OpStat;
use crate::OpWrite;
use crate::Result;
use crate::Scheme;

const WEBHDFS_DEFAULT_ENDPOINT: &str = "http://127.0.0.1:9870";

type AsyncReq = Request<AsyncBody>;
type AsyncResp = Response<IncomingAsyncBody>;

type BlockReq = Request<Body>;
type BlockResp = Response<Body>;

/// WebHDFS's RESTFul API support
///
/// # Configurations
///
/// - `root`: The root path of the WebHDFS service.
/// - `endpoint`: The endpoint of the WebHDFS service.
/// - `username`: The username of the WebHDFS service.
/// - `delegation`: The delegation token for WebHDFS.
///
/// Refer to [`Builder`]'s public API docs for more information
///
/// # Environment
///
/// - `OPENDAL_WEBHDFS_ROOT`
/// - `OPENDAL_WEBHDFS_ENDPOINT`
/// - `OPENDAL_WEBHDFS_USERNAME`
/// - `OPENDAL_WEBHDFS_DELEGATION`
/// - `OPENDAL_WEBHDFS_CSRF`
///
/// # Examples
/// ## Via Environment
///
/// Set the environment variables and then use [`WebHdfs::try_from_env`]:
///
/// ```shell
/// export OPENDAL_WEBHDFS_ROOT=/path/to/dir
/// export OPENDAL_WEBHDFS_DELEGATION=<delegation_token>
/// export OPENDAL_WEBHDFS_USERNAME=<username>
/// export OPENDAL_WEBHDFS_ENDPOINT=localhost:50070
/// export OPENDAL_WEBHDFS_CSRF=X-XSRF-HEADER
/// ```
/// ```no_run
/// use std::sync::Arc;
/// use anyhow::Result;
/// use opendal::Object;
/// use opendal::Operator;
/// use opendal::Scheme;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let op: Operator = Operator::from_env(Scheme::WebHdfs)?;
///     let _: Object = op.object("test_file");
///     Ok(())
/// }
/// ```
/// ## Via Builder
/// ```no_run
/// use std::sync::Arc;
///
/// use anyhow::Result;
/// use opendal::services::WebHdfs;
/// use opendal::Object;
/// use opendal::Operator;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let mut builder = WebHdfs::default();
///     // set the root for s3, all operations will happend under this root
///     //
///     // Note:
///     // if the root is not exists, the builder will automatically create the
///     // root directory for you
///     // if the root exists and is a directory, the builder will continue working
///     // if the root exists and is a folder, the builder will fail on building backend
///     builder.root("/path/to/dir");
///     // set the endpoint of webhdfs namenode, controled by dfs.namenode.http-address
///     // default is http://127.0.0.1:9870
///     builder.endpoint("http://127.0.0.1:9870");

///     // set the delegation_token for builder
///     builder.delegation("delegation_token");
///     // or set the username for builder
///     builder.user("username");
///     // proxy user is also supported
///     builder.doas("proxy_user");
///     // if no delegation token, username and proxy user are set
///     // the backend will query without authentications
///
///     // custom CSRF header is also customizable
///     // default is X-XSRF-HEADER
///     builder.csrf("X-XSRF-HEADER");
///
///     let op: Operator = Operator::new(builder.build()?);
///
///     // create an object handler to start operation on object.
///     let _: Object = op.object("test_file");
///
///     Ok(())
/// }
/// ```
#[derive(Default, Clone)]
pub struct WebHdfsBuilder {
    root: Option<String>,
    endpoint: Option<String>,
    delegation: Option<String>,
    user: Option<String>,
    // proxy user
    doas: Option<String>,
    // custom CSRF prevention header name
    csrf: Option<String>,
}

impl Debug for WebHdfsBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Builder");
        ds.field("root", &self.root)
            .field("endpoint", &self.endpoint);
        if self.delegation.is_some() {
            ds.field("delegation", &"<redacted>");
        }
        if self.user.is_some() {
            ds.field("user", &"<redacted>");
        }
        if self.doas.is_some() {
            ds.field("doas", &"<redacted>");
        }
        if self.csrf.is_some() {
            ds.field("csrf", &"<redacted>");
        }
        ds.finish()
    }
}

impl WebHdfsBuilder {
    /// Set the working directory of this backend
    ///
    /// All operations will happen under this root
    /// # Note
    /// The root will be automatically created if not exists.
    /// If the root is ocupied by a file, building of directory will fail
    pub fn root(&mut self, root: &str) -> &mut Self {
        self.root = if root.is_empty() {
            None
        } else {
            Some(root.to_string())
        };

        self
    }

    /// Set the remote address of this backend
    /// default to `http://127.0.0.1:9870`
    ///
    /// Endpoints should be full uri, e.g.
    ///
    /// - `https://webhdfs.example.com:9870`
    /// - `http://192.168.66.88:9870`
    ///
    /// If user inputs endpoint without scheme, we will
    /// prepend `http://` to it.
    pub fn endpoint(&mut self, endpoint: &str) -> &mut Self {
        if !endpoint.is_empty() {
            // trim tailing slash so we can accept `http://127.0.0.1:9870/`
            self.endpoint = Some(endpoint.trim_end_matches('/').to_string());
        }
        self
    }

    /// Set the delegation token of this backend,
    /// used for authentication
    /// # Note
    /// The builder prefers using delegation token over username.
    /// If both are set, delegation token will be used.
    pub fn delegation(&mut self, delegation: &str) -> &mut Self {
        if !delegation.is_empty() {
            self.delegation = Some(delegation.to_string());
        }
        self
    }

    /// Set the username of this backend,
    /// used for authentication
    /// # Note
    /// The builder prefers using delegation token over username.
    /// If both are set, delegation token will be used. And username
    /// will be ignored.
    pub fn user(&mut self, user: &str) -> &mut Self {
        if user.is_empty() {
            self.user = Some(user.to_string());
        }
        self
    }

    /// Set the proxy user of this backend
    /// # Note
    /// The builder prefers using delegation token,
    /// If both are set, delegation token will be used. And doas
    /// will be ignored
    pub fn doas(&mut self, doas: &str) -> &mut Self {
        if !doas.is_empty() {
            self.doas = Some(doas.to_string());
        }
        self
    }

    /// Set th CSRF prevention header name
    /// refer to [CSRF prevention](https://hadoop.apache.org/docs/r3.3.3/hadoop-project-dist/hadoop-hdfs/WebHDFS.html#Cross-Site_Request_Forgery_Prevention) for detail
    ///
    /// # Note:
    /// If not set, the default value is `X-XSRF-HEADER`
    pub fn csrf(&mut self, csrf: &str) -> &mut Self {
        if !csrf.is_empty() {
            self.csrf = Some(csrf.to_string());
        }
        self
    }
}

impl Builder for WebHdfsBuilder {
    const SCHEME: Scheme = Scheme::WebHdfs;
    type Accessor = WebHdfsBackend;

    fn from_map(map: HashMap<String, String>) -> Self {
        let mut builder = WebHdfsBuilder::default();

        for (k, v) in map.iter() {
            let v = v.as_str();
            match k.as_str() {
                "root" => builder.root(v),
                "endpoint" => builder.endpoint(v),
                "delegation" => builder.delegation(v),
                "user" => builder.user(v),
                "doas" => builder.doas(v),
                "csrf" => builder.csrf(v),
                _ => continue,
            };
        }

        builder
    }

    /// build the backend
    ///
    /// # Note:
    /// when building backend, the built backend will check if the root directory
    /// exits.
    /// if the directory does not exits, the directory will be automatically created
    /// if the root path is occupied by a file, a failure will be returned
    fn build(&mut self) -> Result<Self::Accessor> {
        debug!("building backend: {:?}", self);

        let root = normalize_root(&self.root.take().unwrap_or_default());
        debug!("backend use root {}", root);

        let endpoint = match self.endpoint.take() {
            Some(e) => e,
            None => WEBHDFS_DEFAULT_ENDPOINT.to_string(),
        };
        debug!("backend use endpoint {}", endpoint);

        let csrf = match self.csrf.take() {
            Some(c) => c,
            None => "X-XSRF-HEADER".to_string(),
        };

        let auth = {
            if let Some(token) = self.delegation.take() {
                Some(WebHdfsAuth::new_delegation(&token))
            } else if let Some(username) = self.user.take() {
                let doas = if let Some(doas) = self.doas.take() {
                    doas
                } else {
                    String::new()
                };
                Some(WebHdfsAuth::new_user(&username, &doas))
            } else {
                debug!("neither delegation token nor username is set");
                None
            }
        };
        let auth_str = auth.map(|s| s.auth_str());

        let client = HttpClient::new()?;

        let backend = WebHdfsBackend {
            root: root.clone(),
            endpoint,
            auth: auth_str,
            client,
            csrf,
        };

        debug!("checking working directory: {}", root);
        backend.check_root()?;

        Ok(backend)
    }
}

/// Backend for WebHDFS service
#[derive(Debug, Clone)]
pub struct WebHdfsBackend {
    root: String,
    endpoint: String,
    client: HttpClient,
    auth: Option<String>,
    csrf: String,
}

impl WebHdfsBackend {
    async fn create_or_write_object_req(
        &self,
        path: &str,
        size: Option<u64>,
        content_type: Option<&str>,
        body: AsyncBody,
    ) -> Result<AsyncReq> {
        let p = build_abs_path(&self.root, path);
        let op = if path.ends_with('/') {
            "MKDIRS"
        } else {
            "CREATE"
        };
        let mut url = format!(
            "{}/webhdfs/v1/{}?op={}&overwrite=true&noredirect=true",
            self.endpoint,
            percent_encode_path(&p),
            op,
        );
        if let Some(auth) = &self.auth {
            url = url + "&" + auth;
        }

        let req = Request::put(&url)
            .header(self.csrf.as_str(), "")
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;
        // mkdir does not redirect
        if path.ends_with('/') {
            return Ok(req);
        }

        let resp = self.client.send_async(req).await?;

        self.put_along_redirect(resp, size, content_type, body)
            .await
    }

    async fn open_object_req(&self, path: &str, range: &BytesRange) -> Result<AsyncReq> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=OPEN&noredirect=true",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(auth) = &self.auth {
            url = url + "&" + auth;
        }

        // make a WebHdfs compatible bytes range
        //
        // WebHdfs does not support read from end
        // have to solve manually
        let range = match (range.offset(), range.size()) {
            // avoiding reading the whole file
            (None, Some(size)) => {
                let status = self.stat(path, OpStat::default()).await?;
                let total_size = status.into_metadata().content_length();
                let offset = total_size - size;
                BytesRange::new(Some(offset), Some(size))
            }
            _ => *range,
        };

        let (offset, size) = (range.offset(), range.size());

        match (offset, size) {
            (Some(offset), Some(size)) => {
                url = format!("{url}&offset={offset}&length={size}");
            }
            (Some(offset), None) => {
                url = format!("{url}&offset={offset}");
            }
            (None, None) => {
                // read all, do nothing
            }
            (None, Some(_)) => {
                // already handled
                unreachable!()
            }
        }

        let req = Request::get(&url)
            .header(self.csrf.as_str(), "")
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;

        Ok(req)
    }

    fn status_object_req<B>(&self, path: &str, body: B) -> Result<Request<B>> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=GETFILESTATUS",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(auth) = &self.auth {
            url = url + "&" + auth;
        }

        let req = Request::get(&url);
        req.header(self.csrf.as_str(), "")
            .body(body)
            .map_err(new_request_build_error)
    }

    fn delete_object_req(&self, path: &str) -> Result<AsyncReq> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=DELETE&recursive=false",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(auth) = &self.auth {
            url = url + "&" + auth;
        }

        let req = Request::delete(&url);
        let req = req
            .header(self.csrf.as_str(), "")
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;
        Ok(req)
    }

    fn list_object_req(&self, path: &str) -> Result<AsyncReq> {
        let p = build_abs_path(&self.root, path);
        let mut url = format!(
            "{}/webhdfs/v1/{}?op=LISTSTATUS",
            self.endpoint,
            percent_encode_path(&p),
        );
        if let Some(auth) = &self.auth {
            url = url + "&" + auth;
        }

        let req = Request::get(&url);
        let req = req
            .header(self.csrf.as_str(), "")
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)?;
        Ok(req)
    }
}

impl WebHdfsBackend {
    // blocking create or write object request
    fn blocking_create_root_req(&self) -> Result<BlockReq> {
        let p = &self.root;
        let op = "MKDIRS";
        let mut url = format!(
            "{}/webhdfs/v1/{}?op={}&overwrite=true&noredirect=true",
            self.endpoint,
            percent_encode_path(p),
            op,
        );
        if let Some(auth) = &self.auth {
            url = url + "&" + auth;
        }

        let req = Request::put(&url)
            .header(self.csrf.as_str(), "")
            .body(Body::Empty)
            .map_err(new_request_build_error)?;
        // mkdir does not redirect
        Ok(req)
    }
}

impl WebHdfsBackend {
    /// get object from webhdfs
    /// # Note
    /// looks like webhdfs doesn't support range request from file end.
    /// so if we want to read the tail of object, the whole object should be transfered.
    async fn get_object(&self, path: &str, range: BytesRange) -> Result<AsyncResp> {
        let req = self.open_object_req(path, &range).await?;
        let resp = self.client.send_async(req).await?;

        // this should be an 200 OK http response
        // with JSON redirect message in its body
        if resp.status() != StatusCode::OK {
            // let the outside handle this error
            return Ok(resp);
        }

        let redirected = self.get_along_redirect(resp).await?;
        self.client.send_async(redirected).await
    }

    async fn status_object(&self, path: &str) -> Result<AsyncResp> {
        let req = self.status_object_req(path, AsyncBody::Empty)?;
        self.client.send_async(req).await
    }

    async fn delete_object(&self, path: &str) -> Result<AsyncResp> {
        let req = self.delete_object_req(path)?;
        self.client.send_async(req).await
    }

    /// get redirect destination from 307 TEMPORARY_REDIRECT http response
    async fn follow_redirect(&self, resp: AsyncResp) -> Result<String> {
        let bs = resp.into_body().bytes().await.map_err(|e| {
            Error::new(ErrorKind::Unexpected, "redirection receive fail")
                .with_context("service", Scheme::WebHdfs)
                .set_source(e)
        })?;
        let loc = serde_json::from_reader::<_, Redirection>(bs.reader())
            .map_err(|e| {
                Error::new(ErrorKind::Unexpected, "redirection fail")
                    .with_context("service", Scheme::WebHdfs)
                    .set_permanent()
                    .set_source(e)
            })?
            .location;

        Ok(loc)
    }
}

impl WebHdfsBackend {
    fn blocking_status_object(&self, path: &str) -> Result<BlockResp> {
        let req = self.status_object_req(path, Body::Empty)?;
        self.client.send(req)
    }
}

impl WebHdfsBackend {
    async fn get_along_redirect(&self, redirection: AsyncResp) -> Result<AsyncReq> {
        let redirect = self.follow_redirect(redirection).await?;

        Request::get(redirect)
            .header(self.csrf.as_str(), "")
            .body(AsyncBody::Empty)
            .map_err(new_request_build_error)
    }

    async fn put_along_redirect(
        &self,
        resp: AsyncResp,
        size: Option<u64>,
        content_type: Option<&str>,
        body: AsyncBody,
    ) -> Result<AsyncReq> {
        let redirect = self.follow_redirect(resp).await?;

        let mut req = Request::put(redirect);
        if let Some(size) = size {
            req = req.header(CONTENT_LENGTH, size.to_string());
        }
        if let Some(content_type) = content_type {
            req = req.header(CONTENT_TYPE, content_type);
        }
        req.header(self.csrf.as_str(), "")
            .body(body)
            .map_err(new_request_build_error)
    }

    fn consume_success_mkdir(&self, path: &str, parts: Parts, body: &str) -> Result<RpCreate> {
        let mkdir_rsp = serde_json::from_str::<BooleanResp>(body).map_err(|e| {
            Error::new(ErrorKind::Unexpected, "cannot parse mkdir response")
                .set_temporary()
                .with_context("service", Scheme::WebHdfs)
                .with_context("response", format!("{parts:?}"))
                .set_source(e)
        })?;

        if mkdir_rsp.boolean {
            Ok(RpCreate::default())
        } else {
            return Err(Error::new(
                ErrorKind::Unexpected,
                &format!("mkdir failed: {path}"),
            ));
        }
    }
}

impl WebHdfsBackend {
    fn stat_root(&self) -> Result<ObjectMode> {
        let resp = self.blocking_status_object("/")?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let (_, s) = blocking_resp_to_str(resp)?;

                let file_status = serde_json::from_str::<FileStatusWrapper>(&s)
                    .map_err(|e| {
                        Error::new(ErrorKind::Unexpected, "cannot parse returned json")
                            .with_context("service", Scheme::WebHdfs)
                            .set_source(e)
                    })?
                    .file_status;
                let status_meta: ObjectMetadata = file_status.try_into()?;

                Ok(status_meta.mode())
            }
            _ => Err(blocking_parse_error(resp)?),
        }
    }

    fn create_root(&self) -> Result<RpCreate> {
        let path = "/";
        let req = self.blocking_create_root_req()?;
        let resp = self.client.send(req)?;

        let status = resp.status();

        // WebHDFS's has a two-step create/append to prevent clients to send out
        // data before creating it.
        // According to the redirect policy of `reqwest` HTTP Client we are using,
        // the redirection should be done automatically.
        match status {
            StatusCode::CREATED | StatusCode::OK => {
                let (parts, body) = blocking_resp_to_str(resp)?;
                self.consume_success_mkdir(path, parts, &body)
            }
            _ => Err(blocking_parse_error(resp)?),
        }
    }

    fn check_root(&self) -> Result<()> {
        match self.stat_root() {
            Ok(mode) => {
                if !mode.is_dir() {
                    warn!("working directory is occupied!");
                    return Err(
                        Error::new(ErrorKind::BackendConfigInvalid, "root is occupied!")
                            .with_context("service", Scheme::WebHdfs),
                    );
                }
            }
            Err(e) => match e.kind() {
                ErrorKind::ObjectNotFound => {
                    debug!("working directory does not exists, creating...");
                    self.create_root()?;
                }
                _ => return Err(e),
            },
        }
        debug!("working directory is ready!");
        Ok(())
    }
}

#[async_trait]
impl Accessor for WebHdfsBackend {
    type Reader = IncomingAsyncBody;
    type BlockingReader = ();

    fn metadata(&self) -> AccessorMetadata {
        let mut am = AccessorMetadata::default();
        am.set_scheme(Scheme::WebHdfs)
            .set_root(&self.root)
            .set_capabilities(
                AccessorCapability::Read | AccessorCapability::Write | AccessorCapability::List,
            );
        am
    }

    /// Create a file or directory
    async fn create(&self, path: &str, args: OpCreate) -> Result<RpCreate> {
        // if the path ends with '/', it will be treated as a directory
        // otherwise, it will be treated as a file
        let path = if args.mode().is_file() && path.ends_with('/') {
            path.trim_end_matches('/').to_owned()
        } else if args.mode().is_dir() && !path.ends_with('/') {
            path.to_owned() + "/"
        } else {
            path.to_owned()
        };

        let req = self
            .create_or_write_object_req(&path, Some(0), None, AsyncBody::Empty)
            .await?;

        let resp = self.client.send_async(req).await?;

        let status = resp.status();

        // WebHDFS's has a two-step create/append to prevent clients to send out
        // data before creating it.
        // According to the redirect policy of `reqwest` HTTP Client we are using,
        // the redirection should be done automatically.
        match status {
            StatusCode::CREATED | StatusCode::OK => {
                if !path.ends_with('/') {
                    // create file's http resp could be ignored
                    resp.into_body().consume().await?;
                    return Ok(RpCreate::default());
                }
                let (parts, body) = resp.into_parts();
                let bs = body.bytes().await?;
                let s = String::from_utf8_lossy(&bs);
                self.consume_success_mkdir(&path, parts, &s)
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let range = args.range();
        let resp = self.get_object(path, range).await?;
        match resp.status() {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => {
                let meta = parse_into_object_metadata(path, resp.headers())?;
                Ok((RpRead::with_metadata(meta), resp.into_body()))
            }
            StatusCode::NOT_FOUND => Err(Error::new(ErrorKind::ObjectNotFound, "object not found")
                .with_context("service", Scheme::WebHdfs)),
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn write(&self, path: &str, args: OpWrite, r: input::Reader) -> Result<RpWrite> {
        let req = self
            .create_or_write_object_req(
                path,
                Some(args.size()),
                args.content_type(),
                AsyncBody::Reader(r),
            )
            .await?;
        let resp = self.client.send_async(req).await?;

        let status = resp.status();
        match status {
            StatusCode::OK | StatusCode::CREATED => {
                resp.into_body().consume().await?;
                Ok(RpWrite::default())
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn stat(&self, path: &str, _: OpStat) -> Result<RpStat> {
        let resp = self.status_object(path).await?;
        let status = resp.status();
        match status {
            StatusCode::OK => {
                let mut meta = parse_into_object_metadata(path, resp.headers())?;
                let body_bs = resp.into_body().bytes().await?;

                let file_status = serde_json::from_reader::<_, FileStatusWrapper>(body_bs.reader())
                    .map_err(|e| {
                        Error::new(ErrorKind::Unexpected, "cannot parse returned json")
                            .with_context("service", Scheme::WebHdfs)
                            .set_source(e)
                    })?
                    .file_status;
                let status_meta: ObjectMetadata = file_status.try_into()?;

                // is ok to unwrap here
                // all metadata field of status meta is present and checked by `TryFrom`
                meta.set_last_modified(status_meta.last_modified().unwrap())
                    .set_content_length(status_meta.content_length());
                Ok(RpStat::new(meta))
            }

            _ => Err(parse_error(resp).await?),
        }
    }

    async fn delete(&self, path: &str, _: OpDelete) -> Result<RpDelete> {
        let resp = self.delete_object(path).await?;
        match resp.status() {
            StatusCode::OK => {
                resp.into_body().consume().await?;
                Ok(RpDelete::default())
            }
            _ => Err(parse_error(resp).await?),
        }
    }

    async fn list(&self, path: &str, _: OpList) -> Result<(RpList, ObjectPager)> {
        let path = path.trim_end_matches('/');
        let req = self.list_object_req(path)?;

        let resp = self.client.send_async(req).await?;
        match resp.status() {
            StatusCode::OK => {
                let body_bs = resp.into_body().bytes().await?;
                let file_statuses =
                    serde_json::from_reader::<_, FileStatusesWrapper>(body_bs.reader())
                        .map_err(|e| {
                            Error::new(ErrorKind::Unexpected, "cannot parse returned json")
                                .with_context("service", Scheme::WebHdfs)
                                .set_source(e)
                        })?
                        .file_statuses
                        .file_status;

                let objects = DirStream::new(path, file_statuses);
                Ok((RpList::default(), Box::new(objects) as Box<dyn ObjectPage>))
            }
            StatusCode::NOT_FOUND => {
                let objects = DirStream::new(path, vec![]);
                Ok((RpList::default(), Box::new(objects) as Box<dyn ObjectPage>))
            }
            _ => Err(parse_error(resp).await?),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Redirection {
    pub location: String,
}

#[derive(Debug, Deserialize)]
struct BooleanResp {
    pub boolean: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FileStatusWrapper {
    pub file_status: FileStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FileStatusesWrapper {
    pub file_statuses: FileStatuses,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct FileStatuses {
    pub file_status: Vec<FileStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FileStatus {
    pub length: u64,
    pub modification_time: i64,

    pub path_suffix: String,
    #[serde(rename = "type")]
    pub ty: FileStatusType,
}

impl TryFrom<FileStatus> for ObjectMetadata {
    type Error = Error;
    fn try_from(value: FileStatus) -> Result<Self> {
        let mut meta = match value.ty {
            FileStatusType::Directory => ObjectMetadata::new(ObjectMode::DIR),
            FileStatusType::File => ObjectMetadata::new(ObjectMode::FILE),
        };
        let till_now = time::Duration::milliseconds(value.modification_time);
        let last_modified = time::OffsetDateTime::UNIX_EPOCH
            .checked_add(till_now)
            .ok_or(Error::new(
                ErrorKind::Unexpected,
                "last modification overflowed!",
            ))?;
        meta.set_last_modified(last_modified)
            .set_content_length(value.length);
        Ok(meta)
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub(super) enum FileStatusType {
    Directory,
    File,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_file_statuses() {
        let json = r#"{"Location":"http://<DATANODE>:<PORT>/webhdfs/v1/<PATH>?op=CREATE..."}"#;
        let redir: Redirection = serde_json::from_str(json).expect("must success");
        assert_eq!(
            redir.location,
            "http://<DATANODE>:<PORT>/webhdfs/v1/<PATH>?op=CREATE..."
        );
    }

    #[test]
    fn test_file_status() {
        let json = r#"
{
  "FileStatus":
  {
    "accessTime"      : 0,
    "blockSize"       : 0,
    "group"           : "supergroup",
    "length"          : 0,
    "modificationTime": 1320173277227,
    "owner"           : "webuser",
    "pathSuffix"      : "",
    "permission"      : "777",
    "replication"     : 0,
    "type"            : "DIRECTORY"
  }
}
"#;
        let status: FileStatusWrapper = serde_json::from_str(json).expect("must success");
        assert_eq!(status.file_status.length, 0);
        assert_eq!(status.file_status.modification_time, 1320173277227);
        assert_eq!(status.file_status.path_suffix, "");
        assert_eq!(status.file_status.ty, FileStatusType::Directory);
    }

    #[tokio::test]
    async fn test_list_empty() {
        let json = r#"
    {
        "FileStatuses": {"FileStatus":[]}
    }
        "#;
        let file_statuses = serde_json::from_str::<FileStatusesWrapper>(json)
            .expect("must success")
            .file_statuses
            .file_status;
        assert!(file_statuses.is_empty());
    }

    #[tokio::test]
    async fn test_list_status() {
        let json = r#"
{
  "FileStatuses":
  {
    "FileStatus":
    [
      {
        "accessTime"      : 1320171722771,
        "blockSize"       : 33554432,
        "group"           : "supergroup",
        "length"          : 24930,
        "modificationTime": 1320171722771,
        "owner"           : "webuser",
        "pathSuffix"      : "a.patch",
        "permission"      : "644",
        "replication"     : 1,
        "type"            : "FILE"
      },
      {
        "accessTime"      : 0,
        "blockSize"       : 0,
        "group"           : "supergroup",
        "length"          : 0,
        "modificationTime": 1320895981256,
        "owner"           : "szetszwo",
        "pathSuffix"      : "bar",
        "permission"      : "711",
        "replication"     : 0,
        "type"            : "DIRECTORY"
      }
    ]
  }
}
            "#;

        let file_statuses = serde_json::from_str::<FileStatusesWrapper>(json)
            .expect("must success")
            .file_statuses
            .file_status;

        let mut pager = DirStream::new("listing/directory", file_statuses);
        let mut entries = vec![];
        while let Some(oes) = pager.next_page().await.expect("must success") {
            entries.extend(oes);
        }
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path(), "listing/directory/a.patch");
        assert_eq!(entries[0].mode(), ObjectMode::FILE);
        assert_eq!(entries[1].path(), "listing/directory/bar/");
        assert_eq!(entries[1].mode(), ObjectMode::DIR);
    }
}
