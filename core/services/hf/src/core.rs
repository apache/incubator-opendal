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

use asyncband::mutex::Mutex;
use bytes::Buf;
use bytes::Bytes;
use http::Request;
use http::Response;
use http::StatusCode;
use http::header;
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use xet::xet_session::{XetDownloadStreamGroup, XetSession, XetSessionBuilder, XetUploadCommit};

use opendal_core::raw::*;
use opendal_core::*;

use super::HUGGINGFACE_SCHEME;

/// Repository type of Huggingface. Supports `model`, `dataset`, `space`, and `bucket`.
/// [Reference](https://huggingface.co/docs/hub/repositories)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HfRepoType {
    Model,
    Dataset,
    Space,
    Bucket,
}

impl HfRepoType {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().replace(' ', "").as_str() {
            "model" | "models" => Ok(Self::Model),
            "dataset" | "datasets" => Ok(Self::Dataset),
            "space" | "spaces" => Ok(Self::Space),
            "bucket" | "buckets" => Ok(Self::Bucket),
            other => Err(Error::new(
                ErrorKind::ConfigInvalid,
                format!("unknown repo type: {other}"),
            )
            .with_context("service", HUGGINGFACE_SCHEME)),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Dataset => "dataset",
            Self::Space => "space",
            Self::Bucket => "bucket",
        }
    }

    pub fn as_plural_str(&self) -> &'static str {
        match self {
            Self::Model => "models",
            Self::Dataset => "datasets",
            Self::Space => "spaces",
            Self::Bucket => "buckets",
        }
    }
}

/// Download mode for HuggingFace files.
///
/// - `xet` (default): uses the XET protocol, asks resolve for XET file metadata,
///   and routes XET files through the CAS download stream.
/// - `http`: follows the resolve redirect and streams bytes directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum HfDownloadMode {
    #[default]
    Xet,
    Http,
}

impl HfDownloadMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "xet" => Ok(Self::Xet),
            "http" => Ok(Self::Http),
            other => Err(Error::new(
                ErrorKind::ConfigInvalid,
                format!("unknown download mode: {other}"),
            )
            .with_context("service", HUGGINGFACE_SCHEME)),
        }
    }
}

/// API payload structures for commit operations
#[derive(Debug, serde::Serialize)]
pub(super) struct CommitFile {
    pub path: String,
    pub content: String,
    pub encoding: String,
}

#[derive(Debug, serde::Serialize)]
pub(super) struct LfsFile {
    pub path: String,
    pub oid: String,
    pub algo: String,
    pub size: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct DeletedFile {
    pub path: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct DeletedFolder {
    pub path: String,
}

/// Bucket batch operation payload structures
#[derive(Debug, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum BucketOperation {
    #[serde(rename_all = "camelCase")]
    AddFile { path: String, xet_hash: String },
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    DeleteFile { path: String },
}

#[derive(serde::Serialize)]
pub(super) struct MixedCommitPayload {
    pub summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<CommitFile>,
    #[serde(rename = "lfsFiles", skip_serializing_if = "Vec::is_empty")]
    pub lfs_files: Vec<LfsFile>,
    #[serde(rename = "deletedFiles", skip_serializing_if = "Vec::is_empty")]
    pub deleted_files: Vec<DeletedFile>,
    #[serde(rename = "deletedFolders", skip_serializing_if = "Vec::is_empty")]
    pub deleted_folders: Vec<DeletedFolder>,
}

// API response types

#[derive(Deserialize, Clone)]
pub(super) struct XetFileResponse {
    pub hash: String,
    pub size: u64,
}

/// Response shape of HF's `xet-{read,write}-token` endpoint. Matches
/// `CasJWTInfo` in the vendored `xet` crate (`xet_client::hub_client::types`)
/// — the same three fields regardless of read or write token.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct XetTokenResponse {
    cas_url: String,
    exp: u64,
    access_token: String,
}

/// A cached CAS access token, so a new [`XetDownloadStreamGroup`] doesn't
/// have to fetch its own on every creation.
#[derive(Clone)]
struct XetToken {
    cas_url: String,
    access_token: String,
    /// Unix timestamp (seconds) at which HF says this token actually
    /// expires.
    expires_at: u64,
}

/// How much earlier than a token's real expiry to refresh it ourselves.
/// Larger than the `xet` crate's own internal 30s buffer
/// (`xet_client::cas_client::auth::REFRESH_BUFFER_SEC`) so a group we just
/// built has headroom left, instead of immediately tripping that buffer and
/// refreshing again itself via its own unmocked, uncounted HTTP client.
const XET_TOKEN_REFRESH_BUFFER_SECS: u64 = 120;

/// Which CAS token scope to fetch/cache -- reads and writes are distinct HF
/// API token scopes, each with their own cache slot on [`HfCore`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum XetTokenScope {
    Read,
    Write,
}

impl XetTokenScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }

    /// `Operation` to tag this scope's token fetch with, so a token-less
    /// writer is rejected locally by `request()` instead of hitting the
    /// network.
    fn operation(self) -> Operation {
        match self {
            Self::Read => Operation::Read,
            Self::Write => Operation::Write,
        }
    }
}

#[derive(serde::Deserialize, Debug)]
pub(super) struct CommitResponse {
    #[allow(dead_code)]
    #[serde(rename = "commitOid")]
    pub commit_oid: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "commitUrl")]
    pub commit_url: Option<String>,
}

#[derive(Deserialize, Eq, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct PathInfo {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub oid: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub lfs: Option<LfsInfo>,
    pub path: String,
    #[serde(default)]
    pub last_commit: Option<LastCommit>,
    /// BLAKE3 Merkle hash for XET-stored files; absent for plain git or non-XET LFS files.
    #[serde(rename = "xetHash", default)]
    pub xet_hash: Option<String>,
}

impl PathInfo {
    pub fn entry_mode(&self) -> EntryMode {
        match self.type_.as_str() {
            "directory" => EntryMode::DIR,
            "file" => EntryMode::FILE,
            _ => EntryMode::Unknown,
        }
    }

    pub fn metadata(&self) -> Result<Metadata> {
        let mode = self.entry_mode();
        let mut meta = match mode {
            EntryMode::FILE => MetadataBuilder::file(self.size),
            EntryMode::DIR => MetadataBuilder::dir(),
            EntryMode::Unknown => MetadataBuilder::unknown(),
        };

        if let Some(commit_info) = self.last_commit.as_ref() {
            meta.last_modified(commit_info.date.parse::<Timestamp>()?);
        }

        if mode == EntryMode::FILE {
            // For buckets, oid may be None; for regular repos, prefer lfs.oid then oid
            if let Some(lfs) = &self.lfs {
                meta.etag(&lfs.oid);
            } else if let Some(oid) = &self.oid {
                meta.etag(oid);
            }
        }

        Ok(meta.build())
    }
}

#[derive(Deserialize, Eq, PartialEq, Debug)]
pub(super) struct LfsInfo {
    pub oid: String,
}

#[derive(Deserialize, Eq, PartialEq, Debug)]
pub(super) struct LastCommit {
    pub id: String,
    pub date: String,
}

pub(super) enum HfReadResponse {
    Http(Response<HttpBody>),
    Xet(XetFileResponse),
}

impl HfReadResponse {
    async fn from_response(resp: Response<HttpBody>, mode: HfDownloadMode) -> Result<Self> {
        if mode != HfDownloadMode::Xet || !resp.headers().contains_key("x-xet-hash") {
            return Ok(Self::Http(resp));
        }
        let (_, mut body) = resp.into_parts();
        let buf = body.to_buffer().await?;
        let info = serde_json::from_reader(buf.reader()).map_err(new_json_deserialize_error)?;
        Ok(Self::Xet(info))
    }
}

// Bound entries, paths, and HTTP URIs. Busy entries are never evicted:
// recreating their slots would allow two concurrent resolutions for one path.
const RESOLVED_FILE_MAX_ENTRIES: usize = 512;
const RESOLVED_FILE_MAX_PATH_BYTES: usize = 4096;
const HTTP_DOWNLOAD_MAX_URI_BYTES: usize = 8192;
const HTTP_DOWNLOAD_EXPIRY_MARGIN: Duration = Duration::from_secs(30);

#[derive(Default)]
struct ResolvedFiles {
    entries: StdMutex<HashMap<String, ResolvedFileEntry>>,
}

struct ResolvedFileEntry {
    accessed: Instant,
    state: Arc<Mutex<ResolvedFile>>,
}

#[derive(Default)]
enum ResolvedFile {
    #[default]
    Empty,
    Http(Arc<HttpDownload>),
    Xet(XetFileResponse),
    // The path is not XET-backed and has no reusable HTTP destination.
    NotXet,
    // Wake existing waiters onto the uncached path when a response cannot be
    // admitted. Its slot leaves the table so later reads can try again.
    Bypass,
}

struct HttpDownload {
    redirect: HttpRedirect,
    valid_until: Instant,
}

impl ResolvedFiles {
    fn entry(&self, path: &str) -> Option<Arc<Mutex<ResolvedFile>>> {
        if path.len() > RESOLVED_FILE_MAX_PATH_BYTES {
            return None;
        }

        let mut entries = self
            .entries
            .lock()
            .expect("resolved file cache lock poisoned");
        if entries.len() == RESOLVED_FILE_MAX_ENTRIES && !entries.contains_key(path) {
            let oldest = entries
                .iter()
                .filter(|(_, entry)| Arc::strong_count(&entry.state) == 1)
                .min_by_key(|(_, entry)| entry.accessed)
                .map(|(path, _)| path.clone())?;
            entries.remove(&oldest);
        }
        let entry = entries
            .entry(path.to_string())
            .or_insert_with(|| ResolvedFileEntry {
                accessed: Instant::now(),
                state: Arc::default(),
            });
        entry.accessed = Instant::now();
        Some(entry.state.clone())
    }
}

impl HttpDownload {
    fn from_response(resp: &Response<HttpBody>) -> Option<Self> {
        let redirect = resp.extensions().get::<HttpRedirect>()?;
        let uri = redirect.uri().original_uri().parse::<http::Uri>().ok()?;
        if uri.scheme_str() != Some("https")
            || uri.authority()?.as_str().contains('@')
            || redirect.uri().original_uri().len() > HTTP_DOWNLOAD_MAX_URI_BYTES
        {
            return None;
        }

        // HF's signed CDN destinations advertise their authorization deadline
        // as Expires. Unknown signature formats fall back to fresh resolution.
        let mut expirations = uri.query()?.split('&').filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "Expires").then_some(value)
        });
        let expires = expirations.next()?.parse::<u64>().ok()?;
        if expirations.next().is_some() {
            return None;
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
        let lifetime = Duration::from_secs(expires)
            .checked_sub(now)?
            .checked_sub(HTTP_DOWNLOAD_EXPIRY_MARGIN)?;
        if lifetime.is_zero() {
            return None;
        }

        Some(Self {
            redirect: redirect.clone(),
            valid_until: Instant::now().checked_add(lifetime)?,
        })
    }
}

// Core HuggingFace client that manages API interactions, authentication
// and shared logic for reader/writer/lister.

#[derive(Clone)]
pub struct HfCore {
    pub info: ServiceInfo,
    pub capability: Capability,
    pub repo: HfRepo,
    pub root: String,
    pub token: Option<String>,
    pub endpoint: String,
    pub xet_session: XetSession,
    pub download_mode: HfDownloadMode,
    pub enable_resolve_cache: bool,
    /// Shared file resolutions, keyed by paths relative to this core's root.
    resolved_files: Arc<ResolvedFiles>,
    /// Cached CAS read token, shared by every `XetDownloadStreamGroup` this
    /// core creates, so at most one `xet-read-token` request happens per
    /// token lifetime instead of one per group (one per file read).
    xet_read_token: Arc<Mutex<Option<XetToken>>>,
    /// Same idea for `XetUploadCommit`'s `xet-write-token`. Separate slot:
    /// read and write are distinct HF API token scopes.
    xet_write_token: Arc<Mutex<Option<XetToken>>>,
}

impl Debug for HfCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HfCore")
            .field("repo", &self.repo)
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

impl HfCore {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        info: ServiceInfo,
        capability: Capability,
        repo: HfRepo,
        root: String,
        token: Option<String>,
        endpoint: String,
        xet_session: XetSession,
        download_mode: HfDownloadMode,
    ) -> Self {
        Self {
            info,
            capability,
            repo,
            root,
            token,
            endpoint,
            xet_session,
            download_mode,
            enable_resolve_cache: false,
            resolved_files: Arc::default(),
            xet_read_token: Arc::new(Mutex::new(None)),
            xet_write_token: Arc::new(Mutex::new(None)),
        }
    }

    pub fn build(
        info: ServiceInfo,
        capability: Capability,
        repo: HfRepo,
        root: String,
        token: Option<String>,
        endpoint: String,
        download_mode: HfDownloadMode,
    ) -> Result<Self> {
        let xet_session = XetSessionBuilder::new().build().map_err(|err| {
            Error::new(ErrorKind::Unexpected, "failed to create xet session").set_source(err)
        })?;

        Ok(Self::new(
            info,
            capability,
            repo,
            root,
            token,
            endpoint,
            xet_session,
            download_mode,
        ))
    }

    fn xet_token_refresh_headers(&self) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        if let Some(token) = &self.token
            && let Ok(val) = format!("Bearer {}", token).parse()
        {
            headers.insert(header::AUTHORIZATION, val);
        }
        headers
    }

    /// Create a new XET upload commit, seeded with a cached CAS write token
    /// so it doesn't have to fetch its own. Still creates a fresh commit
    /// each call -- only the token (and its `/api` request) is reused; see
    /// [`Self::xet_download_group`] for why the object itself isn't cached.
    pub(super) async fn xet_upload_commit(
        &self,
        ctx: &OperationContext,
    ) -> Result<XetUploadCommit> {
        let token = self.cached_xet_token(ctx, XetTokenScope::Write).await?;
        self.xet_session
            .new_upload_commit()
            .map_err(|err| {
                Error::new(ErrorKind::Unexpected, "failed to create xet upload commit")
                    .set_source(err)
            })?
            .with_endpoint(token.cas_url)
            .with_token_info(token.access_token, token.expires_at)
            .with_token_refresh_url(
                self.repo
                    .xet_token_url(&self.endpoint, XetTokenScope::Write),
                self.xet_token_refresh_headers(),
            )
            .build()
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Unexpected, "failed to build xet upload commit")
                    .set_source(err)
            })
    }

    /// Get a still-valid cached token for `scope`, fetching and caching a
    /// fresh one if missing or close to expiry. The lock is held across the
    /// refresh request so concurrent callers single-flight onto one fetch.
    ///
    /// On a clock failure, `now` falls back to `u64::MAX` so the token is
    /// treated as stale (extra `/api` traffic) rather than `0`, which would
    /// pin a stale token as valid forever.
    async fn cached_xet_token(
        &self,
        ctx: &OperationContext,
        scope: XetTokenScope,
    ) -> Result<XetToken> {
        let cache = match scope {
            XetTokenScope::Read => &self.xet_read_token,
            XetTokenScope::Write => &self.xet_write_token,
        };
        let mut cached = cache.lock().await;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(u64::MAX);
        if let Some(token) = cached.as_ref()
            && token.expires_at > now.saturating_add(XET_TOKEN_REFRESH_BUFFER_SECS)
        {
            return Ok(token.clone());
        }

        let url = self.repo.xet_token_url(&self.endpoint, scope);
        let req = self
            .request(http::Method::GET, &url, scope.operation(), "XetToken")?
            .body(Buffer::new())
            .map_err(new_request_build_error)?;
        let resp = self.send(ctx, req).await?;
        if !resp.status().is_success() {
            let (parts, _) = resp.into_parts();
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("XetToken")),
                parts,
            ));
        }
        let (_, mut body) = resp.into_parts();
        let buffer = body.to_buffer().await?;
        let info: XetTokenResponse =
            serde_json::from_reader(buffer.reader()).map_err(new_json_deserialize_error)?;
        let fresh = XetToken {
            cas_url: info.cas_url,
            access_token: info.access_token,
            expires_at: info.exp,
        };
        *cached = Some(fresh.clone());
        Ok(fresh)
    }

    /// Create a new XET download stream group, seeded with a cached CAS read
    /// token so it doesn't have to fetch its own. Called once per `HfReader`
    /// (see `xet_group` in reader.rs) rather than cached on `HfCore`: its
    /// internal progress tracking (`GroupProgress.items`) has no eviction, so
    /// sharing one group across every reader on an `Operator` would grow it
    /// without bound. Scoping to a reader's lifetime keeps that growth
    /// bounded by the reader instead. Same reasoning applies to
    /// `xet_upload_commit`'s commit objects.
    pub(super) async fn xet_download_group(
        &self,
        ctx: &OperationContext,
    ) -> Result<XetDownloadStreamGroup> {
        let token = self.cached_xet_token(ctx, XetTokenScope::Read).await?;
        self.xet_session
            .new_download_stream_group()
            .map_err(|err| {
                Error::new(
                    ErrorKind::Unexpected,
                    "failed to create download stream group",
                )
                .set_source(err)
            })?
            .with_endpoint(token.cas_url)
            .with_token_info(token.access_token, token.expires_at)
            .with_token_refresh_url(
                self.repo.xet_token_url(&self.endpoint, XetTokenScope::Read),
                self.xet_token_refresh_headers(),
            )
            .build()
            .await
            .map_err(|err| {
                Error::new(
                    ErrorKind::Unexpected,
                    "failed to build download stream group",
                )
                .set_source(err)
            })
    }

    /// Build an authenticated HTTP request.
    ///
    /// Returns `PermissionDenied` for write operations when no token
    /// is configured.
    pub(super) fn request(
        &self,
        method: http::Method,
        url: &str,
        op: Operation,
        service_operation: &'static str,
    ) -> Result<http::request::Builder> {
        let url = HttpUri::new(url);
        debug!(
            "hf request: service_operation={service_operation} operation={op} method={method} url={}",
            url.redacted_uri()
        );
        let mut req = Request::builder()
            .method(method)
            .uri(url.original_uri())
            .extension(op)
            .extension(ServiceOperation(service_operation));
        match &self.token {
            Some(token) => {
                if let Ok(auth) = format_authorization_by_bearer(token) {
                    req = req.header(header::AUTHORIZATION, auth);
                }
            }
            None if matches!(op, Operation::Write | Operation::Delete) => {
                return Err(Error::new(
                    ErrorKind::PermissionDenied,
                    "token is required for write operations",
                ));
            }
            None => {}
        }
        Ok(req)
    }

    /// Send `req`, following one same-endpoint `307`/`308` redirect.
    ///
    /// HF resolves repo ids case-insensitively but answers any request for
    /// a non-canonically cased id (`user/repo` for `user/Repo`) with a 307
    /// to the canonical URL. Transports follow that for `GET`s but hand a
    /// bodied `POST` (commit, paths-info) back as the bare 307, which would
    /// otherwise surface as an error while reads silently succeed.
    /// Re-issuing the request here makes the outcome independent of the
    /// transport's redirect policy. `send` returns a redirect to any other
    /// host as-is, so it never sends the bearer token off the configured
    /// endpoint itself; the caller then reports it via [`parse_error`].
    pub(super) async fn send(
        &self,
        ctx: &OperationContext,
        req: Request<Buffer>,
    ) -> Result<Response<HttpBody>> {
        let retry = req.clone();

        // The response is confined to this block so it is gone before the
        // retry runs: its body is never read, so the connection cannot be
        // pooled and is better closed than held across another round trip.
        let target = {
            let resp = ctx.http_transport().fetch(req).await?;
            if !matches!(
                resp.status(),
                StatusCode::TEMPORARY_REDIRECT | StatusCode::PERMANENT_REDIRECT
            ) {
                return Ok(resp);
            }

            // Accept a path-only `Location`, which is what HF sends, or one
            // already on the endpoint (a bare origin, normalized by
            // `HfBuilder`). What remains after dropping the endpoint must be
            // a single-slash path, which also refuses a protocol-relative
            // `//host/...` naming another host.
            let Some(location) = parse_location(resp.headers())? else {
                return Ok(resp);
            };
            let path = location
                .strip_prefix(self.endpoint.as_str())
                .unwrap_or(location);
            if !path.starts_with('/') || path.starts_with("//") {
                return Ok(resp);
            }
            format!("{}{path}", self.endpoint)
        };

        let target = HttpUri::new(target);
        debug!("hf request redirected: url={}", target.redacted_uri());
        let (mut parts, body) = retry.into_parts();
        parts.uri = target
            .original_uri()
            .parse()
            .map_err(new_http_uri_invalid_error)?;
        ctx.http_transport()
            .fetch(Request::from_parts(parts, body))
            .await
    }

    /// Convert an operator-relative path to a repo-absolute path
    /// (no leading `/`) for use in commit/delete/batch payloads.
    pub(super) fn repo_path(&self, path: &str) -> String {
        build_abs_path(&self.root, path)
            .trim_start_matches('/')
            .to_string()
    }

    pub(super) async fn path_info(&self, ctx: &OperationContext, path: &str) -> Result<PathInfo> {
        let uri = self.repo.uri(&self.root, path);
        let url = uri.paths_info_url(&self.endpoint);
        let form_body = format!("paths={}&expand=True", percent_encode_path(&uri.path));

        let req = self
            .request(http::Method::POST, &url, Operation::Stat, "PathInfo")?
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Buffer::from(Bytes::from(form_body)))
            .map_err(new_request_build_error)?;
        let resp = self.send(ctx, req).await?;
        if !resp.status().is_success() {
            let (parts, _) = resp.into_parts();
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("PathInfo")),
                parts,
            ));
        }
        let (_, mut body) = resp.into_parts();
        let buffer = body.to_buffer().await?;
        let mut files: Vec<PathInfo> =
            serde_json::from_reader(buffer.reader()).map_err(new_json_deserialize_error)?;

        // NOTE: if the file is not found, the server will return 200 with an empty array
        if files.is_empty() {
            return Err(Error::new(ErrorKind::NotFound, "path not found"));
        }

        Ok(files.remove(0))
    }

    pub(super) async fn read(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
    ) -> Result<HfReadResponse> {
        let entry = if self.enable_resolve_cache {
            self.resolved_files.entry(path)
        } else {
            None
        };
        let Some(entry) = entry else {
            let resp = self.resolve(ctx, path, range, self.download_mode).await?;
            return HfReadResponse::from_response(resp, self.download_mode).await;
        };

        let mut rejected: Option<Arc<HttpDownload>> = None;
        loop {
            let destination = {
                let mut state = entry.lock().await;
                match &*state {
                    ResolvedFile::Xet(info) => return Ok(HfReadResponse::Xet(info.clone())),
                    ResolvedFile::NotXet | ResolvedFile::Bypass => {
                        drop(state);
                        let resp = self.resolve(ctx, path, range, HfDownloadMode::Http).await?;
                        return Ok(HfReadResponse::Http(resp));
                    }
                    ResolvedFile::Http(current)
                        if current.valid_until > Instant::now()
                            && !rejected
                                .as_ref()
                                .is_some_and(|old| Arc::ptr_eq(old, current)) =>
                    {
                        current.clone()
                    }
                    _ => {
                        let mode = if matches!(&*state, ResolvedFile::Http(_)) {
                            HfDownloadMode::Http
                        } else {
                            self.download_mode
                        };
                        // Serialize resolution and refresh. XET metadata is
                        // independent of the range; a one-byte probe bounds the
                        // body discarded when the file turns out to use HTTP.
                        *state = ResolvedFile::Empty;
                        let resolve_range = if mode == HfDownloadMode::Xet {
                            BytesRange::new(0, Some(1))
                        } else {
                            range
                        };
                        let resp = self.resolve(ctx, path, resolve_range, mode).await?;
                        let resp = match HfReadResponse::from_response(resp, mode).await? {
                            HfReadResponse::Xet(info) => {
                                *state = ResolvedFile::Xet(info.clone());
                                return Ok(HfReadResponse::Xet(info));
                            }
                            HfReadResponse::Http(resp) => resp,
                        };
                        *state = match HttpDownload::from_response(&resp) {
                            Some(download) => ResolvedFile::Http(Arc::new(download)),
                            None if self.download_mode == HfDownloadMode::Xet => {
                                ResolvedFile::NotXet
                            }
                            None => {
                                // This live slot cannot have been evicted or
                                // replaced while we hold an Arc to it.
                                self.resolved_files
                                    .entries
                                    .lock()
                                    .expect("resolved file cache lock poisoned")
                                    .remove(path);
                                ResolvedFile::Bypass
                            }
                        };
                        if mode == HfDownloadMode::Http {
                            // The response already contains the requested range.
                            return Ok(HfReadResponse::Http(resp));
                        }
                        // The XET probe returned one byte of HTTP content.
                        // Reuse its destination to fetch the caller's range.
                        continue;
                    }
                }
            };

            let url = self
                .repo
                .uri(&self.root, path)
                .resolve_url(&self.endpoint, self.repo.revision());
            let mut req = self
                .request(http::Method::GET, &url, Operation::Read, "Download")?
                .extension(destination.redirect.clone());
            if !range.is_full() {
                req = req.header(header::RANGE, range.to_header());
            }
            let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
            let resp = ctx.http_transport().fetch(req).await?;
            if resp.status().is_success() {
                return Ok(HfReadResponse::Http(resp));
            }
            if rejected.is_none()
                && matches!(
                    resp.status(),
                    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
                )
            {
                // A late rejection must not discard another caller's refreshed
                // destination. The next iteration compares the Arc generation.
                rejected = Some(destination);
                continue;
            }
            let (parts, _) = resp.into_parts();
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("Download")),
                parts,
            ));
        }
    }

    /// Send `GET /resolve` and return the raw streaming response.
    ///
    /// In `Xet` mode adds `Accept: application/vnd.xet-fileinfo+json` so the
    /// server returns XET metadata instead of redirecting; in `Http` mode the
    /// redirect is followed and the file bytes are streamed directly. Either
    /// way, the response may carry an `x-xet-hash` header -- interpreting
    /// that is up to the caller, not this method's concern.
    pub(super) async fn resolve(
        &self,
        ctx: &OperationContext,
        path: &str,
        range: BytesRange,
        mode: HfDownloadMode,
    ) -> Result<Response<HttpBody>> {
        let uri = self.repo.uri(&self.root, path);
        let url = uri.resolve_url(&self.endpoint, self.repo.revision());

        let mut req = self.request(http::Method::GET, &url, Operation::Read, "Resolve")?;

        if mode == HfDownloadMode::Xet {
            req = req.header(header::ACCEPT, "application/vnd.xet-fileinfo+json");
        }

        if !range.is_full() {
            req = req.header(header::RANGE, range.to_header());
        }

        let req = req.body(Buffer::new()).map_err(new_request_build_error)?;
        let resp = self.send(ctx, req).await?;

        if !resp.status().is_success() {
            let status = resp.status();
            // Drop the streaming body without reading it — parse_error reads
            // only response headers, so there is no need to buffer the body
            // (which may be a large HTML error page).
            let (parts, _) = resp.into_parts();
            let mut err = parse_error(ErrorContext::new(ServiceOperation("Resolve")), parts);
            if status == http::StatusCode::NOT_FOUND && self.path_info(ctx, path).await.is_ok() {
                err = err.set_temporary();
            }
            return Err(err);
        }

        Ok(resp)
    }

    /// Commit file changes to a git-based repo (model/dataset/space).
    ///
    /// Counterpart of [`commit_bucket`](Self::commit_bucket) for bucket repos.
    pub(super) async fn commit_git(
        &self,
        ctx: &OperationContext,
        regular_files: Vec<CommitFile>,
        lfs_files: Vec<LfsFile>,
        deleted_files: Vec<DeletedFile>,
        deleted_folders: Vec<DeletedFolder>,
    ) -> Result<CommitResponse> {
        let url = self.repo.git_commit_url(&self.endpoint);

        let payload = MixedCommitPayload {
            summary: "Commit via OpenDAL".to_string(),
            files: regular_files,
            lfs_files,
            deleted_files,
            deleted_folders,
        };

        let json_body = serde_json::to_vec(&payload).map_err(new_json_serialize_error)?;

        let req = self
            .request(http::Method::POST, &url, Operation::Write, "CommitGit")?
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONTENT_LENGTH, json_body.len())
            .body(Buffer::from(json_body))
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;
        if !resp.status().is_success() {
            let (parts, _) = resp.into_parts();
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CommitGit")),
                parts,
            ));
        }
        let (_, mut body) = resp.into_parts();
        let buffer = body.to_buffer().await?;
        serde_json::from_reader(buffer.reader()).map_err(new_json_deserialize_error)
    }

    /// Commit file changes to a bucket repo via the NDJSON batch API.
    ///
    /// Counterpart of [`commit_git`](Self::commit_git) for git-based repos.
    pub(super) async fn commit_bucket(
        &self,
        ctx: &OperationContext,
        operations: Vec<BucketOperation>,
    ) -> Result<()> {
        if operations.is_empty() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "no operations to perform",
            ));
        }

        let url = self.repo.bucket_batch_url(&self.endpoint);

        let mut body = String::new();
        for op in operations {
            let json = serde_json::to_string(&op).map_err(new_json_serialize_error)?;
            body.push_str(&json);
            body.push('\n');
        }

        let req = self
            .request(http::Method::POST, &url, Operation::Write, "CommitBucket")?
            .header(header::CONTENT_TYPE, "application/x-ndjson")
            .header(header::CONTENT_LENGTH, body.len())
            .body(Buffer::from(Bytes::from(body)))
            .map_err(new_request_build_error)?;

        let resp = self.send(ctx, req).await?;
        if !resp.status().is_success() {
            let (parts, _) = resp.into_parts();
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("CommitBucket")),
                parts,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use http::{Request, Response};
    use std::sync::{Arc, Mutex};

    use super::super::core::HfRepoType;
    use super::*;

    #[derive(Clone)]
    pub(crate) struct MockHttpTransport {
        url: Arc<Mutex<Option<String>>>,
        body: Arc<Mutex<Option<String>>>,
        request_count: Arc<Mutex<usize>>,
        /// `exp` returned by the mocked `xet-{read,write}-token` endpoint.
        xet_token_expires_at: Arc<Mutex<u64>>,
        /// Number of upcoming `fetch` calls that must fail with a 500
        /// before resuming normal responses.
        fail_next: Arc<Mutex<usize>>,
        /// When set, a `/resolve/` request carries `x-xet-hash` and the
        /// mocked [`XetFileResponse`] body instead of plain bytes, so tests
        /// can exercise the XET classification path without real network.
        xet_file: Arc<Mutex<Option<XetFileResponse>>>,
        /// `Range` header of the most recent XET metadata probe, so tests can
        /// check it sends a fixed single byte rather than the caller's range.
        classify_range_header: Arc<Mutex<Option<String>>>,
    }

    impl MockHttpTransport {
        pub(crate) fn new() -> Self {
            Self {
                url: Arc::new(Mutex::new(None)),
                body: Arc::new(Mutex::new(None)),
                request_count: Arc::new(Mutex::new(0)),
                xet_token_expires_at: Arc::new(Mutex::new(u64::MAX)),
                fail_next: Arc::new(Mutex::new(0)),
                xet_file: Arc::new(Mutex::new(None)),
                classify_range_header: Arc::new(Mutex::new(None)),
            }
        }

        /// Controls the `exp` field returned by the mocked
        /// `xet-{read,write}-token` endpoint for subsequent requests.
        pub(crate) fn set_xet_token_expires_at(&self, expires_at: u64) {
            *self.xet_token_expires_at.lock().unwrap() = expires_at;
        }

        /// Makes the next `n` calls to `fetch` return a 500 response instead
        /// of their normal mocked payload.
        pub(crate) fn fail_next_requests(&self, n: usize) {
            *self.fail_next.lock().unwrap() = n;
        }

        /// Makes every subsequent `/resolve/` request report the path as
        /// XET-backed with the given hash/size, via `x-xet-hash` and a
        /// mocked [`XetFileResponse`] body.
        pub(crate) fn set_xet_backed(&self, hash: &str, size: u64) {
            *self.xet_file.lock().unwrap() = Some(XetFileResponse {
                hash: hash.to_string(),
                size,
            });
        }

        pub(crate) fn get_captured_url(&self) -> String {
            self.url.lock().unwrap().clone().unwrap()
        }

        pub(crate) fn get_captured_body(&self) -> String {
            self.body.lock().unwrap().clone().unwrap_or_default()
        }

        pub(crate) fn request_count(&self) -> usize {
            *self.request_count.lock().unwrap()
        }

        pub(crate) fn get_captured_classify_range_header(&self) -> Option<String> {
            self.classify_range_header.lock().unwrap().clone()
        }
    }

    impl HttpTransport for MockHttpTransport {
        async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
            // Yield once so concurrent callers (`futures::join!` in the
            // single-flight tests) actually interleave instead of running to
            // completion one at a time -- without this, a fully synchronous
            // mock never gives the executor a chance to poll a second caller
            // while the first is still "in flight", so tests asserting on
            // single-flighted locking would pass even if the locking were
            // broken.
            tokio::task::yield_now().await;
            *self.request_count.lock().unwrap() += 1;

            {
                let mut fail_next = self.fail_next.lock().unwrap();
                if *fail_next > 0 {
                    *fail_next -= 1;
                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("x-error-message", "mock injected failure")
                        .body(HttpBody::new(futures::stream::empty(), Some(0)))
                        .unwrap());
                }
            }

            let url = req.uri().to_string();
            *self.url.lock().unwrap() = Some(url.clone());
            *self.body.lock().unwrap() = Some(
                String::from_utf8(req.body().to_bytes().to_vec())
                    .expect("request body must be utf-8 for test payloads"),
            );

            if url.contains("/resolve/")
                && let Some(info) = self.xet_file.lock().unwrap().clone()
            {
                *self.classify_range_header.lock().unwrap() = req
                    .headers()
                    .get(header::RANGE)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                let data = Bytes::from(format!(
                    r#"{{"hash":"{}","size":{}}}"#,
                    info.hash, info.size
                ));
                let size = data.len() as u64;
                let buffer = Buffer::from(data);
                return Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_LENGTH, size)
                    .header("x-xet-hash", info.hash)
                    .body(HttpBody::new(
                        futures::stream::iter(vec![Ok(buffer)]),
                        Some(size),
                    ))
                    .unwrap());
            }

            // Return a minimal valid JSON response for API requests
            let (body, content_length) = if url.contains("/paths-info/") || url.contains("/tree/") {
                let data =
                    Bytes::from(r#"[{"type":"file","oid":"abc123","size":100,"path":"test.txt"}]"#);
                let size = data.len() as u64;
                let buffer = Buffer::from(data);
                (
                    HttpBody::new(futures::stream::iter(vec![Ok(buffer)]), Some(size)),
                    size,
                )
            } else if url.contains("/commit/") {
                let data = Bytes::from(r#"{}"#);
                let size = data.len() as u64;
                let buffer = Buffer::from(data);
                (
                    HttpBody::new(futures::stream::iter(vec![Ok(buffer)]), Some(size)),
                    size,
                )
            } else if url.contains("xet-read-token") || url.contains("xet-write-token") {
                let exp = *self.xet_token_expires_at.lock().unwrap();
                let data = Bytes::from(format!(
                    r#"{{"casUrl":"https://cas.example.com","exp":{exp},"accessToken":"mock-token"}}"#
                ));
                let size = data.len() as u64;
                let buffer = Buffer::from(data);
                (
                    HttpBody::new(futures::stream::iter(vec![Ok(buffer)]), Some(size)),
                    size,
                )
            } else {
                let data = Bytes::from_static(b"hello");
                let size = data.len() as u64;
                let buffer = Buffer::from(data);
                (
                    HttpBody::new(futures::stream::iter(vec![Ok(buffer)]), Some(size)),
                    size,
                )
            };

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, content_length)
                .body(body)
                .unwrap())
        }
    }

    pub(crate) fn create_test_core(
        repo_type: HfRepoType,
        repo_id: &str,
        revision: &str,
        endpoint: &str,
    ) -> (HfCore, OperationContext, MockHttpTransport) {
        create_test_core_with(
            MockHttpTransport::new(),
            repo_type,
            repo_id,
            revision,
            endpoint,
        )
    }

    /// Like [`create_test_core`] but with a caller-supplied transport, for
    /// tests that need to script responses the shared mock doesn't.
    pub(crate) fn create_test_core_with<T: HttpTransport + Clone>(
        transport: T,
        repo_type: HfRepoType,
        repo_id: &str,
        revision: &str,
        endpoint: &str,
    ) -> (HfCore, OperationContext, T) {
        let http_transport = HttpTransporter::new(transport.clone());
        let ctx = OperationContext::from_parts(http_transport, Executor::default());

        let info = ServiceInfo::new("hf", "", "");
        let capability = Capability::default();

        let xet_session = XetSessionBuilder::new()
            .build()
            .expect("failed to create xet session");
        let core = HfCore::new(
            info,
            capability,
            HfRepo::new(repo_type, repo_id.to_string(), Some(revision.to_string())),
            "/".to_string(),
            None,
            endpoint.to_string(),
            xet_session,
            HfDownloadMode::Xet,
        );

        (core, ctx, transport)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use http::Response;

    use super::super::core::HfRepoType;
    use super::test_utils::{create_test_core, create_test_core_with};
    use super::*;

    #[tokio::test]
    async fn test_hf_path_info_url_model() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );

        core.path_info(&ctx, "test.txt").await?;

        let url = mock_client.get_captured_url();
        assert_eq!(
            url,
            "https://huggingface.co/api/models/test-user/test-repo/paths-info/main"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_hf_path_info_url_dataset() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Dataset,
            "test-org/test-dataset",
            "v1.0.0",
            "https://huggingface.co",
        );

        core.path_info(&ctx, "data/file.csv").await?;

        let url = mock_client.get_captured_url();
        assert_eq!(
            url,
            "https://huggingface.co/api/datasets/test-org/test-dataset/paths-info/v1%2E0%2E0"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_hf_path_info_url_custom_endpoint() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-org/test-dataset",
            "refs/convert/parquet",
            "https://custom-hf.example.com",
        );

        core.path_info(&ctx, "model.bin").await?;

        let url = mock_client.get_captured_url();
        assert_eq!(
            url,
            "https://custom-hf.example.com/api/models/test-org/test-dataset/paths-info/refs%2Fconvert%2Fparquet"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_hf_path_info_url_space() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Space,
            "test-user/test-space",
            "main",
            "https://huggingface.co",
        );

        core.path_info(&ctx, "app.py").await?;

        let url = mock_client.get_captured_url();
        assert_eq!(
            url,
            "https://huggingface.co/api/spaces/test-user/test-space/paths-info/main"
        );

        Ok(())
    }

    struct SeenRequest {
        method: String,
        uri: String,
        auth: String,
        body: String,
    }

    /// A scripted transport mirroring how HF serves a repo whose configured
    /// id is not canonically cased: it answers that URL with `status` and a
    /// `Location` of `location`, and only the canonically cased URL accepts
    /// the commit. Tests set both, so one transport covers
    /// a followed redirect, a refused one, and a non-307 status.
    #[derive(Clone)]
    struct RedirectingTransport {
        status: StatusCode,
        location: &'static str,
        requests: Arc<Mutex<Vec<SeenRequest>>>,
    }

    impl HttpTransport for RedirectingTransport {
        async fn fetch(&self, req: Request<Buffer>) -> Result<Response<HttpBody>> {
            let uri = req.uri().to_string();
            let auth = req
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string();
            let body = String::from_utf8(req.body().to_bytes().to_vec()).unwrap();
            self.requests.lock().unwrap().push(SeenRequest {
                method: req.method().to_string(),
                uri: uri.clone(),
                auth,
                body,
            });

            let resp = match uri.as_str() {
                "https://huggingface.co/api/models/test-user/uppercase-repo/commit/main" => {
                    Response::builder()
                        .status(self.status)
                        .header(header::LOCATION, self.location)
                        .header(header::CONTENT_LENGTH, 0)
                        .body(HttpBody::new(futures::stream::empty(), Some(0)))
                }
                "https://huggingface.co/api/models/test-user/Uppercase-Repo/commit/main" => {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_LENGTH, 2)
                        .body(HttpBody::new(
                            futures::stream::iter(vec![Ok(Buffer::from(Bytes::from_static(
                                b"{}",
                            )))]),
                            Some(2),
                        ))
                }
                other => panic!("unexpected request to {other}"),
            };
            Ok(resp.unwrap())
        }
    }

    fn redirecting_core(
        status: StatusCode,
        location: &'static str,
    ) -> (HfCore, OperationContext, RedirectingTransport) {
        let (mut core, ctx, transport) = create_test_core_with(
            RedirectingTransport {
                status,
                location,
                requests: Arc::default(),
            },
            HfRepoType::Model,
            "test-user/uppercase-repo",
            "main",
            "https://huggingface.co",
        );
        core.token = Some("hf_dummy".to_string());
        (core, ctx, transport)
    }

    fn lfs_file(path: &str) -> LfsFile {
        LfsFile {
            path: path.to_string(),
            oid: "deadbeef".to_string(),
            algo: "sha256".to_string(),
            size: 2812,
        }
    }

    /// `send` re-issues a bodied `POST` answered with a 307 to the
    /// canonically cased URL with the same method, headers and body,
    /// regardless of whether the transport replays redirects itself. HF
    /// sends the `Location` as an absolute path, as in the report for #8107.
    #[tokio::test]
    async fn test_commit_follows_case_redirect() -> Result<()> {
        let (core, ctx, transport) = redirecting_core(
            StatusCode::TEMPORARY_REDIRECT,
            "/api/models/test-user/Uppercase-Repo/commit/main",
        );

        core.commit_git(&ctx, vec![], vec![lfs_file("a.md")], vec![], vec![])
            .await?;

        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let (first, redirected) = (&requests[0], &requests[1]);
        assert_eq!(first.method, "POST");
        assert_eq!(
            first.uri,
            "https://huggingface.co/api/models/test-user/uppercase-repo/commit/main"
        );
        assert_eq!(first.auth, "Bearer hf_dummy");
        assert_eq!(redirected.method, "POST");
        assert_eq!(
            redirected.uri,
            "https://huggingface.co/api/models/test-user/Uppercase-Repo/commit/main"
        );
        assert_eq!(redirected.auth, "Bearer hf_dummy");
        assert_eq!(redirected.body, first.body);
        assert!(first.body.contains("a.md"));

        Ok(())
    }

    /// `send` also follows an absolute `Location` on the configured endpoint,
    /// and a 308 like a 307.
    #[tokio::test]
    async fn test_commit_follows_absolute_same_endpoint_redirect() -> Result<()> {
        let (core, ctx, transport) = redirecting_core(
            StatusCode::PERMANENT_REDIRECT,
            "https://huggingface.co/api/models/test-user/Uppercase-Repo/commit/main",
        );

        core.commit_git(&ctx, vec![], vec![lfs_file("a.md")], vec![], vec![])
            .await?;

        assert_eq!(transport.requests.lock().unwrap().len(), 2);
        Ok(())
    }

    /// `send` must not follow a redirect off the configured endpoint: the
    /// re-issued request would carry the bearer token to a foreign host.
    /// The 307 surfaces as an error instead.
    #[tokio::test]
    async fn test_commit_does_not_follow_foreign_redirect() -> Result<()> {
        let (core, ctx, transport) = redirecting_core(
            StatusCode::TEMPORARY_REDIRECT,
            "https://evil.example.com/api/models/test-user/Uppercase-Repo/commit/main",
        );

        let err = core
            .commit_git(&ctx, vec![], vec![lfs_file("a.md")], vec![], vec![])
            .await
            .expect_err("a foreign redirect must not be followed");
        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(
            err.to_string()
                .contains("redirect to https://evil.example.com/api/models/test-user/Uppercase-Repo/commit/main not followed"),
            "the refused redirect must be named in the error: {err}"
        );
        assert_eq!(transport.requests.lock().unwrap().len(), 1);

        Ok(())
    }

    /// A protocol-relative `Location` names another host, so it is refused
    /// rather than treated as a path on ours.
    #[tokio::test]
    async fn test_commit_does_not_follow_protocol_relative_redirect() -> Result<()> {
        let (core, ctx, transport) =
            redirecting_core(StatusCode::TEMPORARY_REDIRECT, "//evil.example.com/api/x");

        let err = core
            .commit_git(&ctx, vec![], vec![lfs_file("a.md")], vec![], vec![])
            .await
            .expect_err("a protocol-relative redirect must not be followed");
        assert!(err.to_string().contains("not followed"), "{err}");
        assert_eq!(transport.requests.lock().unwrap().len(), 1);

        Ok(())
    }

    /// A `Location` without a leading slash is a relative reference this
    /// service does not resolve; it is refused rather than guessed at.
    #[tokio::test]
    async fn test_commit_does_not_follow_relative_redirect() -> Result<()> {
        let (core, ctx, transport) = redirecting_core(
            StatusCode::TEMPORARY_REDIRECT,
            "api/models/test-user/Uppercase-Repo/commit/main",
        );

        let err = core
            .commit_git(&ctx, vec![], vec![lfs_file("a.md")], vec![], vec![])
            .await
            .expect_err("a relative redirect must not be followed");
        assert!(err.to_string().contains("not followed"), "{err}");
        assert_eq!(transport.requests.lock().unwrap().len(), 1);

        Ok(())
    }

    /// `send` only replays 307/308. A 302 is passed through untouched: in
    /// http download mode the transport follows the resolve 302 to the CDN
    /// itself, and `send` must not re-issue it on the API host.
    #[tokio::test]
    async fn test_send_leaves_302_to_the_transport() -> Result<()> {
        let (core, ctx, transport) = redirecting_core(
            StatusCode::FOUND,
            "/api/models/test-user/Uppercase-Repo/commit/main",
        );

        let err = core
            .commit_git(&ctx, vec![], vec![lfs_file("a.md")], vec![], vec![])
            .await
            .expect_err("an unfollowed 302 must surface as an error");
        assert!(
            err.to_string().contains(
                "redirect to /api/models/test-user/Uppercase-Repo/commit/main not followed"
            ),
            "{err}"
        );
        assert_eq!(transport.requests.lock().unwrap().len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_xet_read_token_is_cached_across_calls() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        mock_client.set_xet_token_expires_at(u64::MAX);

        let first = core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(mock_client.request_count(), 1);
        assert_eq!(first.access_token, "mock-token");
        assert_eq!(first.cas_url, "https://cas.example.com");

        // A second call with the cached token nowhere near expiry must not
        // hit the network again.
        let second = core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(mock_client.request_count(), 1);
        assert_eq!(second.access_token, first.access_token);

        Ok(())
    }

    #[tokio::test]
    async fn test_xet_read_token_refreshes_once_stale() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        // Already within the refresh buffer of "now" -- immediately stale.
        mock_client.set_xet_token_expires_at(0);

        core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(mock_client.request_count(), 1);

        core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(
            mock_client.request_count(),
            2,
            "a token that's always stale must be refreshed on every call, not cached"
        );

        Ok(())
    }

    /// A failed refresh must not poison the cache: the next call should
    /// retry against the network rather than being permanently stuck on an
    /// error or on a stale/absent token.
    #[tokio::test]
    async fn test_cached_xet_token_retries_after_fetch_failure() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        mock_client.set_xet_token_expires_at(u64::MAX);
        mock_client.fail_next_requests(1);

        match core.cached_xet_token(&ctx, XetTokenScope::Read).await {
            Err(err) => assert!(err.to_string().contains("mock injected failure")),
            Ok(_) => panic!("a failed token fetch must surface as an error"),
        }
        assert_eq!(mock_client.request_count(), 1);

        let token = core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(token.access_token, "mock-token");
        assert_eq!(mock_client.request_count(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_xet_read_and_write_tokens_are_cached_independently() -> Result<()> {
        let (mut core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        // Write-scope token fetches are tagged `Operation::Write`, which
        // requires a token.
        core.token = Some("hf_dummy".to_string());
        mock_client.set_xet_token_expires_at(u64::MAX);

        core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(mock_client.request_count(), 1);

        // A write token request must not be satisfied by the read token's
        // cache slot -- read and write are distinct HF API scopes.
        core.cached_xet_token(&ctx, XetTokenScope::Write).await?;
        assert_eq!(mock_client.request_count(), 2);
        assert!(mock_client.get_captured_url().contains("write"));

        // Both are now warm; neither call should hit the network again.
        core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        core.cached_xet_token(&ctx, XetTokenScope::Write).await?;
        assert_eq!(mock_client.request_count(), 2);

        Ok(())
    }

    /// The point of caching isn't just `cached_xet_token` in isolation --
    /// `xet_download_group`/`xet_upload_commit` must actually go through it.
    /// A regression that bypassed the cache in their wiring wouldn't be
    /// caught by the `cached_xet_token`-only tests above.
    #[tokio::test]
    async fn test_xet_download_group_reuses_cached_read_token() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        mock_client.set_xet_token_expires_at(u64::MAX);

        core.xet_download_group(&ctx).await?;
        assert_eq!(mock_client.request_count(), 1);

        // A second group build must reuse the cached read token rather than
        // fetching its own.
        core.xet_download_group(&ctx).await?;
        assert_eq!(mock_client.request_count(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_xet_upload_commit_reuses_cached_write_token() -> Result<()> {
        let (mut core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        core.token = Some("hf_dummy".to_string());
        mock_client.set_xet_token_expires_at(u64::MAX);

        core.xet_upload_commit(&ctx).await?;
        assert_eq!(mock_client.request_count(), 1);
        assert!(mock_client.get_captured_url().contains("write"));

        // A second commit build must reuse the cached write token rather
        // than fetching its own.
        core.xet_upload_commit(&ctx).await?;
        assert_eq!(mock_client.request_count(), 1);

        Ok(())
    }

    /// A write-token fetch must be tagged `Operation::Write` so a
    /// token-less writer is rejected locally instead of hitting the network.
    #[tokio::test]
    async fn test_xet_upload_commit_without_token_fails_locally() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        mock_client.set_xet_token_expires_at(u64::MAX);

        match core.xet_upload_commit(&ctx).await {
            Err(err) => assert_eq!(err.kind(), ErrorKind::PermissionDenied),
            Ok(_) => panic!("an upload commit without a token must fail locally"),
        }
        assert_eq!(mock_client.request_count(), 0);

        Ok(())
    }

    /// Locks in the strict `>` in `cached_xet_token`'s freshness check: a
    /// token expiring exactly at the edge of the refresh buffer counts as
    /// stale (refetched), not fresh.
    #[tokio::test]
    async fn test_token_at_refresh_buffer_boundary_is_stale() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Exactly at the buffer boundary: `expires_at > now + BUFFER` is
        // false, so this must be treated as stale.
        mock_client.set_xet_token_expires_at(now + XET_TOKEN_REFRESH_BUFFER_SECS);
        core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(mock_client.request_count(), 1);
        core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(
            mock_client.request_count(),
            2,
            "a token expiring exactly at the refresh buffer must be refetched, not reused"
        );

        // One second past the boundary: now fresh, so cached and reused.
        mock_client.set_xet_token_expires_at(now + XET_TOKEN_REFRESH_BUFFER_SECS + 1);
        core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(mock_client.request_count(), 3);
        core.cached_xet_token(&ctx, XetTokenScope::Read).await?;
        assert_eq!(
            mock_client.request_count(),
            3,
            "a token expiring past the refresh buffer must be cached and reused"
        );

        Ok(())
    }

    /// The whole point of holding the cache's mutex across the refresh
    /// call: concurrent callers racing on a stale/missing token queue
    /// behind one in-flight refresh instead of each firing their own.
    #[tokio::test]
    async fn test_concurrent_stale_token_refresh_is_single_flighted() -> Result<()> {
        let (core, ctx, mock_client) = create_test_core(
            HfRepoType::Model,
            "test-user/test-repo",
            "main",
            "https://huggingface.co",
        );
        mock_client.set_xet_token_expires_at(u64::MAX);

        let (r1, r2, r3) = futures::join!(
            core.cached_xet_token(&ctx, XetTokenScope::Read),
            core.cached_xet_token(&ctx, XetTokenScope::Read),
            core.cached_xet_token(&ctx, XetTokenScope::Read),
        );
        r1?;
        r2?;
        r3?;

        assert_eq!(
            mock_client.request_count(),
            1,
            "concurrent callers racing on a cold/stale token must share one refresh"
        );

        Ok(())
    }

    #[test]
    fn test_parse_error_branch_update_conflict_is_temporary() {
        let (parts, _) = Response::builder()
            .status(StatusCode::PRECONDITION_FAILED)
            .header(
                "x-error-message",
                "The branch was updated since you opened this page. Please refresh and try again.",
            )
            .body(())
            .unwrap()
            .into_parts();

        let err = parse_error(ErrorContext::new(ServiceOperation("Test")), parts);

        assert_eq!(err.kind(), ErrorKind::Conflict);
        assert!(err.is_temporary());
    }

    #[test]
    fn test_parse_error_other_precondition_failed_is_not_temporary() {
        let (parts, _) = Response::builder()
            .status(StatusCode::PRECONDITION_FAILED)
            .header("x-error-message", "etag mismatch")
            .body(())
            .unwrap()
            .into_parts();

        let err = parse_error(ErrorContext::new(ServiceOperation("Test")), parts);

        assert_eq!(err.kind(), ErrorKind::Unexpected);
        assert!(!err.is_temporary());
    }
}

/// Context needed to classify an error from this service.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrorContext {
    service_operation: ServiceOperation,
}

impl ErrorContext {
    pub(crate) const fn new(service_operation: ServiceOperation) -> Self {
        Self { service_operation }
    }
}

/// Parse an error response using its service request context.
pub(crate) fn parse_error(ctx: ErrorContext, mut parts: http::response::Parts) -> Error {
    let location = HttpUri::from_response_location(&mut parts).cloned();
    // HF sets x-error-message on every error response with a short human-readable
    // description. Using the header avoids reading the response body, which can be
    // a large HTML error page (e.g. 52 KB on 404s from the /resolve/ endpoint).
    let error_message = parts
        .headers
        .get("x-error-message")
        .and_then(|v| v.to_str().ok());
    let message = match (error_message, location) {
        (Some(message), _) => message.to_string(),
        (None, Some(location)) if parts.status.is_redirection() => {
            let location = location.redacted_uri();
            format!("redirect to {location} not followed")
        }
        _ => "unknown error".to_string(),
    };

    // HF git-style commit APIs reject stale branch snapshots with 412.
    // Treat this specific conflict as temporary so RetryLayer can replay
    // the whole write/delete close sequence on a fresh branch head.
    let branch_updated_conflict = parts.status == StatusCode::PRECONDITION_FAILED
        && message
            .to_ascii_lowercase()
            .contains("branch was updated since you opened this page");

    let (kind, retryable) = match parts.status {
        StatusCode::NOT_FOUND => (ErrorKind::NotFound, false),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (ErrorKind::PermissionDenied, false),
        StatusCode::PRECONDITION_FAILED if branch_updated_conflict => (ErrorKind::Conflict, true),
        StatusCode::PRECONDITION_FAILED => (ErrorKind::Unexpected, false),
        StatusCode::TOO_MANY_REQUESTS => (ErrorKind::RateLimited, true),
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => (ErrorKind::Unexpected, true),
        _ => (ErrorKind::Unexpected, false),
    };

    let mut err = Error::new(kind, message);

    err = err.with_context("service_operation", ctx.service_operation.0);
    err = with_error_response_context(err, parts);

    if retryable {
        err = err.set_temporary();
    }

    err
}

mod uri {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

    pub use super::HfRepoType;
    use super::XetTokenScope;
    use crate::HUGGINGFACE_SCHEME;
    use opendal_core::raw::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HfRepo {
        pub repo_type: HfRepoType,
        pub repo_id: String,
        pub revision: Option<String>,
    }

    impl HfRepo {
        pub fn new(repo_type: HfRepoType, repo_id: String, revision: Option<String>) -> Self {
            Self {
                repo_type,
                repo_id,
                // An empty revision is no revision: `hf://datasets/user/repo@`
                // and `revision=""` via options both reach here, and leaving
                // `Some("")` in place would build URLs with an empty revision
                // segment instead of falling back to `main`.
                revision: revision.filter(|revision| !revision.is_empty()),
            }
        }

        /// Whether this repo is a bucket (as opposed to a git-based repo).
        pub fn is_bucket(&self) -> bool {
            self.repo_type == HfRepoType::Bucket
        }

        /// Return the revision, defaulting to "main" if unset.
        pub fn revision(&self) -> &str {
            self.revision.as_deref().unwrap_or("main")
        }

        /// Create an `HfUri` for the given root and path within this repo.
        pub fn uri(&self, root: &str, path: &str) -> HfUri {
            HfUri {
                repo: self.clone(),
                path: build_abs_path(root, path)
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .to_string(),
            }
        }

        /// Build the paths-info API URL for this repository.
        pub fn paths_info_url(&self, endpoint: &str) -> String {
            match self.repo_type {
                HfRepoType::Bucket => {
                    format!("{}/api/buckets/{}/paths-info", endpoint, self.repo_id)
                }
                _ => {
                    format!(
                        "{}/api/{}/{}/paths-info/{}",
                        endpoint,
                        self.repo_type.as_plural_str(),
                        self.repo_id,
                        percent_encode_revision(self.revision()),
                    )
                }
            }
        }

        /// Build the XET token API URL for this repository.
        pub fn xet_token_url(&self, endpoint: &str, scope: XetTokenScope) -> String {
            let token_type = scope.as_str();
            match self.repo_type {
                HfRepoType::Bucket => {
                    format!(
                        "{}/api/buckets/{}/xet-{}-token",
                        endpoint, self.repo_id, token_type
                    )
                }
                _ => {
                    format!(
                        "{}/api/{}/{}/xet-{}-token/{}",
                        endpoint,
                        self.repo_type.as_plural_str(),
                        self.repo_id,
                        token_type,
                        self.revision(),
                    )
                }
            }
        }

        /// Build the bucket batch API URL for this repository.
        pub fn bucket_batch_url(&self, endpoint: &str) -> String {
            format!("{}/api/buckets/{}/batch", endpoint, self.repo_id)
        }

        /// Build the git commit API URL for this repository.
        pub fn git_commit_url(&self, endpoint: &str) -> String {
            format!(
                "{}/api/{}/{}/commit/{}",
                endpoint,
                self.repo_type.as_plural_str(),
                self.repo_id,
                percent_encode_revision(self.revision()),
            )
        }
    }

    /// Parsed Hugging Face URI following the official format:
    /// `hf://[<repo_type_prefix>/]<repo_id>[@<revision>][/<path_in_repo>]`
    ///
    /// Use this directly when you need access to `path_in_repo` separately
    /// from the config (e.g. to resolve a specific file within the repo).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HfUri {
        pub repo: HfRepo,
        pub path: String,
    }

    impl HfUri {
        /// Parse a Hugging Face path into its components.
        /// Path format: `[<repo_type>/]<repo_id>[@<revision>][/<path_in_repo>]`
        pub fn parse(path: &str) -> opendal_core::Result<Self> {
            if path.is_empty() {
                return Err(opendal_core::Error::new(
                    opendal_core::ErrorKind::ConfigInvalid,
                    "repo_id is required in uri path",
                )
                .with_context("service", HUGGINGFACE_SCHEME));
            }

            let mut path = path.to_string();

            // Strip repo_type prefix if present (e.g. "datasets/user/repo" → "user/repo")
            let repo_type = if let Some((first, rest)) = path.split_once('/') {
                if let Ok(rt) = HfRepoType::parse(first) {
                    path = rest.to_string();
                    rt
                } else {
                    HfRepoType::Model
                }
            } else if HfRepoType::parse(&path).is_ok() {
                return Err(opendal_core::Error::new(
                    opendal_core::ErrorKind::ConfigInvalid,
                    "repository name is required in uri path",
                )
                .with_context("service", HUGGINGFACE_SCHEME));
            } else {
                HfRepoType::Model
            };

            // Parse repo_id, revision, and path_in_repo.
            // Path is now: <repo_id>[@<revision>][/<path_in_repo>]
            let (repo_id, revision, path_in_repo) = if path.contains('/') {
                // Check if @ appears in the first two segments (the repo_id portion).
                // This distinguishes "user/repo@rev/file" from "user/repo/path/to/@file".
                let first_two: String = path.splitn(3, '/').take(2).collect::<Vec<_>>().join("/");

                if first_two.contains('@') {
                    let (repo_id, rev_and_path) = path.split_once('@').unwrap();
                    let rev_and_path = rev_and_path.replace("%2F", "/");
                    let (revision, path_in_repo) = Self::parse_revision(&rev_and_path);
                    (repo_id.to_string(), Some(revision), path_in_repo)
                } else {
                    let segments: Vec<_> = path.splitn(3, '/').collect();
                    let repo_id = format!("{}/{}", segments[0], segments[1]);
                    let path_in_repo = segments.get(2).copied().unwrap_or("").to_string();
                    (repo_id, None, path_in_repo)
                }
            } else if let Some((repo_id, rev)) = path.split_once('@') {
                let rev = rev.replace("%2F", "/");
                (repo_id.to_string(), Some(rev), String::new())
            } else {
                (path, None, String::new())
            };

            Ok(Self {
                repo: HfRepo::new(repo_type, repo_id, revision),
                path: path_in_repo,
            })
        }

        /// Split a string after `@` into (revision, path_in_repo).
        /// Handles special refs like `refs/convert/parquet` and `refs/pr/10`.
        fn parse_revision(rev_and_path: &str) -> (String, String) {
            if !rev_and_path.contains('/') {
                return (rev_and_path.to_string(), String::new());
            }

            // Match special refs: refs/(convert|pr)/<segment>
            if let Some(rest) = rev_and_path.strip_prefix("refs/convert/") {
                return match rest.split_once('/') {
                    Some((segment, path)) => (format!("refs/convert/{segment}"), path.to_string()),
                    None => (rev_and_path.to_string(), String::new()),
                };
            }
            if let Some(rest) = rev_and_path.strip_prefix("refs/pr/") {
                return match rest.split_once('/') {
                    Some((segment, path)) => (format!("refs/pr/{segment}"), path.to_string()),
                    None => (rev_and_path.to_string(), String::new()),
                };
            }

            // Regular revision: split on first /
            let (rev, path) = rev_and_path.split_once('/').unwrap();
            (rev.to_string(), path.to_string())
        }

        /// Return the revision, defaulting to "main" if unset.
        pub fn revision(&self) -> &str {
            self.repo.revision()
        }

        /// Build the resolve URL for this URI using an explicit revision (e.g. a commit OID).
        ///
        /// Pinning to a specific commit OID avoids CDN consistency lag that can occur
        /// when using a branch name like "main" immediately after a commit.
        pub fn resolve_url(&self, endpoint: &str, revision: &str) -> String {
            let revision = percent_encode_revision(revision);
            let path = percent_encode_path(&self.path);
            match self.repo.repo_type {
                HfRepoType::Model => {
                    format!(
                        "{}/{}/resolve/{}/{}",
                        endpoint, self.repo.repo_id, revision, path
                    )
                }
                HfRepoType::Dataset => {
                    format!(
                        "{}/datasets/{}/resolve/{}/{}",
                        endpoint, self.repo.repo_id, revision, path
                    )
                }
                HfRepoType::Space => {
                    format!(
                        "{}/spaces/{}/resolve/{}/{}",
                        endpoint, self.repo.repo_id, revision, path
                    )
                }
                HfRepoType::Bucket => {
                    format!(
                        "{}/buckets/{}/resolve/{}",
                        endpoint, self.repo.repo_id, path
                    )
                }
            }
        }

        /// Build the paths-info API URL for this URI.
        pub fn paths_info_url(&self, endpoint: &str) -> String {
            self.repo.paths_info_url(endpoint)
        }

        /// Build the file tree API URL for this URI.
        pub fn file_tree_url(
            &self,
            endpoint: &str,
            recursive: bool,
            cursor: Option<&str>,
        ) -> String {
            // HF answers a trailing slash with a 302 to the slash-less URL, so
            // the separator lives in the segment rather than the template.
            let path_segment = if self.path.is_empty() {
                String::new()
            } else {
                format!("/{}", percent_encode_path(&self.path))
            };

            let mut url = if self.repo.is_bucket() {
                format!(
                    "{}/api/buckets/{}/tree{}?expand=True",
                    endpoint, self.repo.repo_id, path_segment,
                )
            } else {
                format!(
                    "{}/api/{}/{}/tree/{}{}?expand=True",
                    endpoint,
                    self.repo.repo_type.as_plural_str(),
                    self.repo.repo_id,
                    percent_encode_revision(self.revision()),
                    path_segment,
                )
            };

            if recursive {
                url.push_str("&recursive=True");
            } else if self.repo.is_bucket() {
                // Bucket tree API defaults to recursive; must opt out explicitly.
                url.push_str("&recursive=false");
            }

            if let Some(cursor_val) = cursor {
                url.push_str(&format!("&cursor={}", cursor_val));
            }

            url
        }
    }

    pub(crate) fn percent_encode_revision(revision: &str) -> String {
        utf8_percent_encode(revision, NON_ALPHANUMERIC).to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn resolve(path: &str) -> HfUri {
            HfUri::parse(path).unwrap()
        }

        #[test]
        fn test_repo_type_parse() {
            assert_eq!(HfRepoType::parse("models").unwrap(), HfRepoType::Model);
            assert_eq!(HfRepoType::parse("Models").unwrap(), HfRepoType::Model);
            assert_eq!(HfRepoType::parse("MODELS").unwrap(), HfRepoType::Model);
            assert_eq!(HfRepoType::parse("datasets").unwrap(), HfRepoType::Dataset);
            assert_eq!(HfRepoType::parse("Datasets").unwrap(), HfRepoType::Dataset);
            assert_eq!(HfRepoType::parse("spaces").unwrap(), HfRepoType::Space);
            assert_eq!(HfRepoType::parse("Spaces").unwrap(), HfRepoType::Space);
            assert_eq!(HfRepoType::parse("model").unwrap(), HfRepoType::Model);
            assert_eq!(HfRepoType::parse("dataset").unwrap(), HfRepoType::Dataset);
            assert_eq!(HfRepoType::parse("space").unwrap(), HfRepoType::Space);
            assert_eq!(HfRepoType::parse("data sets").unwrap(), HfRepoType::Dataset);
            assert_eq!(HfRepoType::parse("Data Sets").unwrap(), HfRepoType::Dataset);
            assert!(HfRepoType::parse("unknown").is_err());
            assert!(HfRepoType::parse("foobar").is_err());
        }

        #[test]
        fn resolve_with_namespace() {
            let p = resolve("username/my_model");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "username/my_model");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_with_revision() {
            let p = resolve("username/my_model@dev");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "username/my_model");
            assert_eq!(p.repo.revision.as_deref(), Some("dev"));
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_datasets_prefix() {
            let p = resolve("datasets/username/my_dataset");
            assert_eq!(p.repo.repo_type, HfRepoType::Dataset);
            assert_eq!(p.repo.repo_id, "username/my_dataset");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_datasets_prefix_and_revision() {
            let p = resolve("datasets/username/my_dataset@dev");
            assert_eq!(p.repo.repo_type, HfRepoType::Dataset);
            assert_eq!(p.repo.repo_id, "username/my_dataset");
            assert_eq!(p.repo.revision.as_deref(), Some("dev"));
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_with_path_in_repo() {
            let p = resolve("username/my_model/config.json");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "username/my_model");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "config.json");
        }

        #[test]
        fn resolve_with_revision_and_path() {
            let p = resolve("username/my_model@dev/path/to/file.txt");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "username/my_model");
            assert_eq!(p.repo.revision.as_deref(), Some("dev"));
            assert_eq!(p.path, "path/to/file.txt");
        }

        #[test]
        fn resolve_datasets_revision_and_path() {
            let p = resolve("datasets/username/my_dataset@dev/train/data.csv");
            assert_eq!(p.repo.repo_type, HfRepoType::Dataset);
            assert_eq!(p.repo.repo_id, "username/my_dataset");
            assert_eq!(p.repo.revision.as_deref(), Some("dev"));
            assert_eq!(p.path, "train/data.csv");
        }

        #[test]
        fn resolve_refs_convert_revision() {
            let p = resolve("datasets/squad@refs/convert/parquet");
            assert_eq!(p.repo.repo_type, HfRepoType::Dataset);
            assert_eq!(p.repo.repo_id, "squad");
            assert_eq!(p.repo.revision.as_deref(), Some("refs/convert/parquet"));
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_refs_convert_revision_with_path() {
            let p = resolve("datasets/squad@refs/convert/parquet/default/train/0000.parquet");
            assert_eq!(p.repo.repo_type, HfRepoType::Dataset);
            assert_eq!(p.repo.repo_id, "squad");
            assert_eq!(p.repo.revision.as_deref(), Some("refs/convert/parquet"));
            assert_eq!(p.path, "default/train/0000.parquet");
        }

        #[test]
        fn resolve_refs_pr_revision() {
            let p = resolve("username/my_model@refs/pr/10");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "username/my_model");
            assert_eq!(p.repo.revision.as_deref(), Some("refs/pr/10"));
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_encoded_revision() {
            let p = resolve("username/my_model@refs%2Fpr%2F10");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "username/my_model");
            assert_eq!(p.repo.revision.as_deref(), Some("refs/pr/10"));
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_at_in_path_not_revision() {
            let p = resolve("username/my_model/path/to/@not-a-revision.txt");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "username/my_model");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "path/to/@not-a-revision.txt");
        }

        #[test]
        fn resolve_bare_repo_type_fails() {
            assert!(HfUri::parse("datasets").is_err());
            assert!(HfUri::parse("").is_err());
        }

        #[test]
        fn resolve_bare_repo_no_namespace() {
            let p = resolve("gpt2");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "gpt2");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_bare_repo_with_revision() {
            let p = resolve("gpt2@dev");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "gpt2");
            assert_eq!(p.repo.revision.as_deref(), Some("dev"));
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_bare_dataset_no_namespace() {
            let p = resolve("datasets/squad");
            assert_eq!(p.repo.repo_type, HfRepoType::Dataset);
            assert_eq!(p.repo.repo_id, "squad");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_bare_dataset_with_revision() {
            let p = resolve("datasets/squad@dev");
            assert_eq!(p.repo.repo_type, HfRepoType::Dataset);
            assert_eq!(p.repo.repo_id, "squad");
            assert_eq!(p.repo.revision.as_deref(), Some("dev"));
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_models_prefix() {
            let p = resolve("models/username/my_model");
            assert_eq!(p.repo.repo_type, HfRepoType::Model);
            assert_eq!(p.repo.repo_id, "username/my_model");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_spaces_prefix() {
            let p = resolve("spaces/username/my_space");
            assert_eq!(p.repo.repo_type, HfRepoType::Space);
            assert_eq!(p.repo.repo_id, "username/my_space");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_buckets_prefix() {
            let p = resolve("buckets/username/my_bucket");
            assert_eq!(p.repo.repo_type, HfRepoType::Bucket);
            assert_eq!(p.repo.repo_id, "username/my_bucket");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "");
        }

        #[test]
        fn resolve_buckets_with_path() {
            let p = resolve("buckets/username/my_bucket/data/file.txt");
            assert_eq!(p.repo.repo_type, HfRepoType::Bucket);
            assert_eq!(p.repo.repo_id, "username/my_bucket");
            assert!(p.repo.revision.is_none());
            assert_eq!(p.path, "data/file.txt");
        }

        #[test]
        fn test_bucket_resolve_url() {
            let p = resolve("buckets/user/bucket/file.txt");
            let url = p.resolve_url("https://huggingface.co", p.revision());
            assert_eq!(
                url,
                "https://huggingface.co/buckets/user/bucket/resolve/file.txt"
            );
        }

        #[test]
        fn test_bucket_xet_token_urls() {
            let p = resolve("buckets/user/bucket");
            let read_url = p
                .repo
                .xet_token_url("https://huggingface.co", XetTokenScope::Read);
            let write_url = p
                .repo
                .xet_token_url("https://huggingface.co", XetTokenScope::Write);
            assert_eq!(
                read_url,
                "https://huggingface.co/api/buckets/user/bucket/xet-read-token"
            );
            assert_eq!(
                write_url,
                "https://huggingface.co/api/buckets/user/bucket/xet-write-token"
            );
        }

        #[test]
        fn test_bucket_batch_url() {
            let p = resolve("buckets/user/bucket");
            let url = p.repo.bucket_batch_url("https://huggingface.co");
            assert_eq!(url, "https://huggingface.co/api/buckets/user/bucket/batch");
        }

        #[test]
        fn test_file_tree_url_root_has_no_trailing_slash() {
            let p = resolve("datasets/user/repo");
            assert_eq!(
                p.file_tree_url("https://huggingface.co", false, None),
                "https://huggingface.co/api/datasets/user/repo/tree/main?expand=True"
            );
        }

        #[test]
        fn test_file_tree_url_subdir_keeps_path_separator() {
            let p = resolve("datasets/user/repo/data");
            assert_eq!(
                p.file_tree_url("https://huggingface.co", false, None),
                "https://huggingface.co/api/datasets/user/repo/tree/main/data?expand=True"
            );
        }

        #[test]
        fn test_file_tree_url_encoded_revision_root() {
            let p = resolve("datasets/user/repo@refs/convert/parquet");
            assert_eq!(
                p.file_tree_url("https://huggingface.co", false, None),
                "https://huggingface.co/api/datasets/user/repo/tree/refs%2Fconvert%2Fparquet?expand=True"
            );
        }

        /// Every construction site funnels through `HfRepo::new`, so an empty
        /// revision from a URI or from `revision=""` in options is unset.
        #[test]
        fn test_empty_revision_is_unset() {
            assert!(resolve("datasets/user/repo@").repo.revision.is_none());
            assert!(
                HfRepo::new(
                    HfRepoType::Dataset,
                    "user/repo".to_string(),
                    Some(String::new()),
                )
                .revision
                .is_none()
            );
        }

        #[test]
        fn test_file_tree_url_recursive_and_cursor() {
            let p = resolve("datasets/user/repo");
            assert_eq!(
                p.file_tree_url("https://huggingface.co", true, Some("abc123")),
                "https://huggingface.co/api/datasets/user/repo/tree/main?expand=True&recursive=True&cursor=abc123"
            );
        }

        #[test]
        fn test_bucket_file_tree_url_root_ends_at_tree() {
            let p = resolve("buckets/user/bucket");
            assert_eq!(
                p.file_tree_url("https://huggingface.co", false, None),
                "https://huggingface.co/api/buckets/user/bucket/tree?expand=True&recursive=false"
            );
        }

        #[test]
        fn test_bucket_file_tree_url_subdir_keeps_path_separator() {
            let p = resolve("buckets/user/bucket/data");
            assert_eq!(
                p.file_tree_url("https://huggingface.co", false, None),
                "https://huggingface.co/api/buckets/user/bucket/tree/data?expand=True&recursive=false"
            );
        }
    }
}

pub(super) use uri::*;
