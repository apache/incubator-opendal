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

use std::num::NonZeroU32;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use http::HeaderMap;
use http::HeaderValue;
use http::header;
use serde_json::Value;
use url::Url;

use opendal_core::raw::oio::ReadStream;
use opendal_core::raw::*;
use opendal_core::*;

const GIT_HEAD_REF: &str = "HEAD";
const LFS_VERSION_PREFIX: &str = "version https://git-lfs.github.com/spec/v1";
const LFS_CONTENT_TYPE: &str = "application/vnd.git-lfs+json";
const LFS_OID_PREFIX: &str = "oid sha256:";
const LFS_SIZE_PREFIX: &str = "size ";
const LFS_ACTION_REUSE_CAP: Duration = Duration::from_secs(60);
const LFS_ACTION_EXPIRY_MARGIN: Duration = Duration::from_secs(5);
const LFS_BATCH_BODY_LIMIT: usize = 1024 * 1024;

/// Parsed Git LFS pointer.
#[derive(Debug, Clone)]
pub(super) struct LfsPointer {
    pub(super) oid: String,
    pub(super) size: u64,
}

/// Repository and its temporary object database.
#[derive(Clone)]
struct RepoHolder {
    repo: gix::Repository,
    _tempdir: Arc<tempfile::TempDir>,
}

/// Resolved LFS download action.
#[derive(Clone)]
pub(super) struct LfsAction {
    pub(super) url: Url,
    pub(super) headers: HeaderMap,
    reuse_until: Instant,
}

impl LfsAction {
    pub(super) fn is_fresh(&self, now: Instant) -> bool {
        self.reuse_until > now
    }
}

/// Shared Git service state.
pub(super) struct GitCore {
    repository: String,
    reference: String,
    root: String,
    username: Option<String>,
    password: Option<String>,
    resolve_lfs: bool,
    repo_cell: Arc<Mutex<Option<RepoHolder>>>,
    commit_oid_cache: Arc<Mutex<Option<gix::hash::ObjectId>>>,
    commit_info_cache: Arc<Mutex<Option<(gix::hash::ObjectId, i64)>>>,
}

impl std::fmt::Debug for GitCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitCore")
            .field("repository", &self.repository)
            .field("reference", &self.reference)
            .field("root", &self.root)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("resolve_lfs", &self.resolve_lfs)
            .finish_non_exhaustive()
    }
}

impl GitCore {
    pub(super) fn new(
        repository: String,
        reference: String,
        root: String,
        username: Option<String>,
        password: Option<String>,
        resolve_lfs: bool,
    ) -> Self {
        Self {
            repository,
            reference,
            root,
            username,
            password,
            resolve_lfs,
            repo_cell: Arc::new(Mutex::new(None)),
            commit_oid_cache: Arc::new(Mutex::new(None)),
            commit_info_cache: Arc::new(Mutex::new(None)),
        }
    }

    pub(super) fn resolve_lfs(&self) -> bool {
        self.resolve_lfs
    }

    fn get_or_create_repo(&self) -> Result<gix::Repository> {
        let mut cell = self
            .repo_cell
            .lock()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "repository lock poisoned"))?;

        if cell.is_none() {
            let temp_dir = tempfile::tempdir().map_err(|_| {
                Error::new(
                    ErrorKind::Unexpected,
                    "failed to create temporary git directory",
                )
            })?;

            let username = self.username.clone();
            let password = self.password.clone();
            let mut prepare = gix::prepare_clone(self.repository.as_str(), temp_dir.path())
                .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to prepare git clone"))?;

            if self.reference == GIT_HEAD_REF {
                let depth = NonZeroU32::new(1).expect("1 is non-zero");
                prepare = prepare.with_shallow(gix::remote::fetch::Shallow::DepthAtRemote(depth));
            }

            prepare = prepare.configure_connection(
                move |conn| -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    if let (Some(username), Some(password)) = (username.clone(), password.clone()) {
                        conn.set_credentials(move |action| match action {
                            gix::credentials::helper::Action::Get(ctx) => {
                                Ok(Some(gix::credentials::protocol::Outcome {
                                    identity: gix::sec::identity::Account {
                                        username: username.clone(),
                                        password: password.clone(),
                                        oauth_refresh_token: None,
                                    },
                                    next: ctx.into(),
                                }))
                            }
                            _ => Ok(None),
                        });
                    }
                    Ok(())
                },
            );

            let (repo, _) = prepare
                .fetch_only(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
                .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to fetch git repository"))?;

            *cell = Some(RepoHolder {
                repo,
                _tempdir: Arc::new(temp_dir),
            });
        }

        cell.as_ref()
            .map(|holder| holder.repo.clone())
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "repository not initialized"))
    }

    fn resolve_reference(&self, repo: &gix::Repository) -> Result<gix::hash::ObjectId> {
        if let Some(oid) = *self
            .commit_oid_cache
            .lock()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "commit cache lock poisoned"))?
        {
            return Ok(oid);
        }

        let object = repo
            .rev_parse_single(self.reference.as_str())
            .map_err(|_| Error::new(ErrorKind::NotFound, "git reference not found"))?
            .object()
            .map_err(|_| Error::new(ErrorKind::NotFound, "git reference object not found"))?;
        let commit_oid = object
            .peel_to_commit()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "git reference is not a commit"))?
            .id()
            .detach();

        *self
            .commit_oid_cache
            .lock()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "commit cache lock poisoned"))? =
            Some(commit_oid);
        Ok(commit_oid)
    }

    fn get_commit_tree<'a>(
        &self,
        repo: &'a gix::Repository,
        commit_oid: gix::hash::ObjectId,
    ) -> Result<(gix::Tree<'a>, i64)> {
        if let Some((tree_oid, commit_time)) = *self
            .commit_info_cache
            .lock()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "commit info cache lock poisoned"))?
        {
            let tree = repo
                .find_object(tree_oid)
                .map_err(|_| Error::new(ErrorKind::NotFound, "tree not found"))?
                .try_into_tree()
                .map_err(|_| Error::new(ErrorKind::Unexpected, "cached object is not a tree"))?;
            return Ok((tree, commit_time));
        }

        let commit = repo
            .find_object(commit_oid)
            .map_err(|_| Error::new(ErrorKind::NotFound, "commit not found"))?
            .peel_to_commit()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "object is not a commit"))?;
        let commit_time = commit
            .time()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to read commit time"))?
            .seconds;
        let tree = commit
            .tree()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to read commit tree"))?;

        *self
            .commit_info_cache
            .lock()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "commit info cache lock poisoned"))? =
            Some((tree.id().detach(), commit_time));
        Ok((tree, commit_time))
    }

    pub(super) fn stat_file_blocking(&self, path: &str) -> Result<(u64, bool, i64)> {
        let repo = self.get_or_create_repo()?;
        let commit_oid = self.resolve_reference(&repo)?;
        let (tree, commit_time) = self.get_commit_tree(&repo, commit_oid)?;

        let rooted_path = build_rooted_abs_path(&self.root, path);
        let path = rooted_path.trim_matches('/');
        if path.is_empty() {
            return Ok((0, true, commit_time));
        }
        let entry = tree
            .lookup_entry_by_path(path)
            .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to look up git path"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "path not found"))?;

        if entry.mode().is_tree() {
            return Ok((0, true, commit_time));
        }
        if entry.mode().is_commit() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "submodule entries are not supported",
            ));
        }

        let blob = entry
            .object()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to read git object"))?
            .try_into_blob()
            .map_err(|_| Error::new(ErrorKind::Unsupported, "git entry is not a blob"))?;
        let mut size = blob.data.len() as u64;
        if self.resolve_lfs && Self::is_lfs_pointer(&blob.data) {
            if let Ok(pointer) = Self::parse_lfs_pointer(&blob.data) {
                size = pointer.size;
            }
        }

        Ok((size, false, commit_time))
    }

    pub(super) fn list_dir_blocking(&self, path: &str) -> Result<Vec<(String, bool, u64, i64)>> {
        let repo = self.get_or_create_repo()?;
        let commit_oid = self.resolve_reference(&repo)?;
        let (mut tree, commit_time) = self.get_commit_tree(&repo, commit_oid)?;

        let rooted_path = build_rooted_abs_path(&self.root, path);
        let path = rooted_path.trim_matches('/');
        if !path.is_empty() {
            let Some(entry) = tree
                .lookup_entry_by_path(path)
                .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to look up git path"))?
            else {
                return Ok(Vec::new());
            };
            if !entry.mode().is_tree() {
                return Err(Error::new(
                    ErrorKind::NotADirectory,
                    "path is not a directory",
                ));
            }
            tree = entry
                .object()
                .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to read git object"))?
                .try_into_tree()
                .map_err(|_| Error::new(ErrorKind::Unexpected, "git entry is not a tree"))?;
        }

        let mut entries = Vec::new();
        for entry in tree.iter() {
            let entry = entry
                .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to read tree entry"))?;
            let name = entry.filename().to_string();

            if entry.mode().is_tree() {
                entries.push((name, true, 0, commit_time));
                continue;
            }
            if entry.mode().is_commit() {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "submodule entries are not supported",
                ));
            }

            let blob = entry
                .object()
                .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to read git object"))?
                .try_into_blob()
                .map_err(|_| Error::new(ErrorKind::Unsupported, "git entry is not a blob"))?;
            let mut size = blob.data.len() as u64;
            if self.resolve_lfs && Self::is_lfs_pointer(&blob.data) {
                if let Ok(pointer) = Self::parse_lfs_pointer(&blob.data) {
                    size = pointer.size;
                }
            }
            entries.push((name, false, size, commit_time));
        }

        Ok(entries)
    }

    pub(super) fn read_blob_blocking(&self, path: &str) -> Result<Bytes> {
        let repo = self.get_or_create_repo()?;
        let commit_oid = self.resolve_reference(&repo)?;
        let (tree, _) = self.get_commit_tree(&repo, commit_oid)?;

        let rooted_path = build_rooted_abs_path(&self.root, path);
        let path = rooted_path.trim_matches('/');
        let entry = tree
            .lookup_entry_by_path(path)
            .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to look up git path"))?
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "path not found"))?;

        if entry.mode().is_tree() {
            return Err(Error::new(ErrorKind::IsADirectory, "path is a directory"));
        }
        if entry.mode().is_commit() {
            return Err(Error::new(
                ErrorKind::Unsupported,
                "submodule entries are not supported",
            ));
        }

        let blob = entry
            .object()
            .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to read git object"))?
            .try_into_blob()
            .map_err(|_| Error::new(ErrorKind::Unsupported, "git entry is not a blob"))?;
        Ok(Bytes::copy_from_slice(&blob.data))
    }

    pub(super) fn is_lfs_pointer(content: &[u8]) -> bool {
        let prefix_len = content.len().min(256);
        std::str::from_utf8(&content[..prefix_len])
            .map(|text| text.starts_with(LFS_VERSION_PREFIX))
            .unwrap_or(false)
    }

    pub(super) fn parse_lfs_pointer(content: &[u8]) -> Result<LfsPointer> {
        let text = std::str::from_utf8(content)
            .map_err(|_| Error::new(ErrorKind::Unexpected, "invalid LFS pointer"))?;
        let mut lines = text.lines();

        if lines.next() != Some(LFS_VERSION_PREFIX) {
            return Err(Error::new(ErrorKind::Unexpected, "invalid LFS pointer"));
        }
        let oid_line = lines
            .next()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "invalid LFS pointer"))?;
        let size_line = lines
            .next()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "invalid LFS pointer"))?;
        if lines.next().is_some() {
            return Err(Error::new(ErrorKind::Unexpected, "invalid LFS pointer"));
        }

        let oid = oid_line
            .strip_prefix(LFS_OID_PREFIX)
            .filter(|oid| {
                oid.len() == 64
                    && oid
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            })
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "invalid LFS pointer"))?;
        let size = size_line
            .strip_prefix(LFS_SIZE_PREFIX)
            .and_then(|size| size.parse::<u64>().ok())
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "invalid LFS pointer"))?;

        Ok(LfsPointer {
            oid: oid.to_string(),
            size,
        })
    }

    pub(super) fn slice_blob(content: Bytes, range: BytesRange) -> Result<(RpRead, Bytes)> {
        let size = content.len() as u64;
        let rp = RpRead::new(MetadataBuilder::file(size).build());
        if range.is_full() {
            return Ok((rp, content));
        }

        let offset = range.offset();
        if offset > size {
            return Err(Error::new(
                ErrorKind::RangeNotSatisfied,
                "range start offset exceeds content length",
            ));
        }
        let end = range
            .size()
            .map(|len| offset.saturating_add(len).min(size))
            .unwrap_or(size);
        Ok((rp, content.slice(offset as usize..end as usize)))
    }

    fn repository_url(&self) -> Result<Url> {
        let url = Url::parse(&self.repository)
            .map_err(|_| Error::new(ErrorKind::ConfigInvalid, "repository URL is invalid"))?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "repository URL must not contain userinfo",
            ));
        }
        Ok(url)
    }

    fn lfs_batch_url(&self) -> Result<Url> {
        let mut url = self.repository_url()?;
        if url.scheme() != "https" {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "LFS resolution requires an https repository URL",
            ));
        }
        if url.query().is_some() || url.fragment().is_some() {
            return Err(Error::new(
                ErrorKind::ConfigInvalid,
                "repository URL must not contain a query or fragment",
            ));
        }

        let path = url.path().trim_end_matches('/');
        let path = if path.ends_with(".git") {
            format!("{path}/info/lfs/objects/batch")
        } else {
            format!("{path}.git/info/lfs/objects/batch")
        };
        url.set_path(&path);
        url.set_query(None);
        url.set_fragment(None);
        Ok(url)
    }

    pub(super) async fn resolve_lfs_action(
        &self,
        ctx: &OperationContext,
        oid: &str,
        size: u64,
    ) -> Result<LfsAction> {
        let batch_url = self.lfs_batch_url()?;
        let body = serde_json::json!({
            "operation": "download",
            "transfers": ["basic"],
            "objects": [{"oid": oid, "size": size}],
        });
        let body = serde_json::to_vec(&body)
            .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to encode LFS request"))?;

        let mut builder = http::Request::builder()
            .method(http::Method::POST)
            .uri(batch_url.as_str())
            .header(header::CONTENT_TYPE, LFS_CONTENT_TYPE)
            .header(header::ACCEPT, LFS_CONTENT_TYPE);
        if let Some(auth) = self.basic_authorization_for(&batch_url) {
            builder = builder.header(header::AUTHORIZATION, auth);
        }
        let request = builder
            .body(Buffer::from(body))
            .map_err(|_| Error::new(ErrorKind::Unexpected, "failed to build LFS batch request"))?;

        let response = ctx.http_transport().fetch(request).await?;
        if !response.status().is_success() {
            return Err(Error::new(
                ErrorKind::Unexpected,
                format!("LFS batch request failed with status {}", response.status()),
            ));
        }

        let mut body = response.into_body();
        let mut bufs = Vec::new();
        let mut total = 0;
        loop {
            let buf = body.read().await?;
            if buf.is_empty() {
                break;
            }
            total += buf.len();
            if total > LFS_BATCH_BODY_LIMIT {
                return Err(Error::new(
                    ErrorKind::Unexpected,
                    "LFS batch response too large",
                ));
            }
            bufs.push(buf);
        }
        let buffer: Buffer = bufs.into_iter().flatten().collect();
        Self::parse_lfs_action(&buffer.to_bytes(), oid, size)
    }

    fn parse_lfs_action(body: &[u8], oid: &str, size: u64) -> Result<LfsAction> {
        let document: Value = serde_json::from_slice(body)
            .map_err(|_| Error::new(ErrorKind::Unexpected, "invalid LFS batch response"))?;
        let objects = document
            .get("objects")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "invalid LFS batch response"))?;
        if objects.len() != 1 {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "invalid LFS batch response",
            ));
        }

        let object = &objects[0];
        if object.get("oid").and_then(Value::as_str) != Some(oid)
            || object.get("size").and_then(Value::as_u64) != Some(size)
        {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "LFS batch response object mismatch",
            ));
        }

        let action = object
            .get("actions")
            .and_then(Value::as_object)
            .and_then(|actions| actions.get("download"))
            .and_then(Value::as_object)
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "LFS download action is missing"))?;
        let href = action
            .get("href")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "invalid LFS download URL"))?;
        let url = Url::parse(href)
            .map_err(|_| Error::new(ErrorKind::Unexpected, "invalid LFS download URL"))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(Error::new(
                ErrorKind::Unexpected,
                "invalid LFS download URL",
            ));
        }

        let mut headers = HeaderMap::new();
        match action.get("header") {
            None | Some(Value::Null) => {}
            Some(value) => {
                let values = value.as_object().ok_or_else(|| {
                    Error::new(ErrorKind::Unexpected, "invalid LFS action headers")
                })?;
                for (name, value) in values {
                    let name =
                        http::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                            Error::new(ErrorKind::Unexpected, "invalid LFS action headers")
                        })?;
                    let value = value.as_str().ok_or_else(|| {
                        Error::new(ErrorKind::Unexpected, "invalid LFS action headers")
                    })?;
                    let value = HeaderValue::from_str(value).map_err(|_| {
                        Error::new(ErrorKind::Unexpected, "invalid LFS action headers")
                    })?;
                    headers.insert(name, value);
                }
            }
        }

        let expires_in = action.get("expires_in").and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        });
        let expires_at = action.get("expires_at").and_then(Value::as_str);
        let now = Instant::now();
        let reuse_until = compute_reuse_until(expires_in, expires_at, now, SystemTime::now());

        Ok(LfsAction {
            url,
            headers,
            reuse_until,
        })
    }

    pub(super) fn action_headers(&self, action: &LfsAction) -> HeaderMap {
        let mut headers = action.headers.clone();
        if !headers.contains_key(header::AUTHORIZATION) {
            if let Some(auth) = self.basic_authorization_for(&action.url) {
                if let Ok(value) = HeaderValue::from_str(&auth) {
                    headers.insert(header::AUTHORIZATION, value);
                }
            }
        }
        headers
    }

    fn basic_authorization_for(&self, target: &Url) -> Option<String> {
        let username = self.username.as_ref()?;
        let password = self.password.as_ref()?;
        let repository = self.repository_url().ok()?;
        if target.origin() != repository.origin() {
            return None;
        }
        format_authorization_by_basic(username, password).ok()
    }
}

fn compute_reuse_until(
    expires_in: Option<u64>,
    expires_at: Option<&str>,
    now: Instant,
    now_system: SystemTime,
) -> Instant {
    let mut advertised = expires_in.map(Duration::from_secs);
    if let Some(expires_at) = expires_at {
        if let Ok(timestamp) = expires_at.parse::<Timestamp>() {
            let expires_at: SystemTime = timestamp.into();
            let duration = expires_at
                .duration_since(now_system)
                .unwrap_or(Duration::ZERO);
            advertised = Some(
                advertised
                    .map(|current| current.min(duration))
                    .unwrap_or(duration),
            );
        }
    }

    let lifetime = advertised
        .map(|duration| duration.saturating_sub(LFS_ACTION_EXPIRY_MARGIN))
        .unwrap_or(LFS_ACTION_REUSE_CAP);
    now + lifetime.min(LFS_ACTION_REUSE_CAP)
}

pub(super) fn join_error(err: tokio::task::JoinError) -> Error {
    Error::new(ErrorKind::Unexpected, "git task failed").set_source(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn action(href: &str, headers: &[(&str, &str)]) -> LfsAction {
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            map.insert(
                http::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        LfsAction {
            url: Url::parse(href).unwrap(),
            headers: map,
            reuse_until: Instant::now(),
        }
    }

    fn core(username: Option<&str>, password: Option<&str>) -> GitCore {
        GitCore::new(
            "https://example.com/repo".to_string(),
            "HEAD".to_string(),
            "/".to_string(),
            username.map(ToString::to_string),
            password.map(ToString::to_string),
            true,
        )
    }

    #[test]
    fn parse_pointer_rejects_uppercase_oid() {
        let pointer = format!(
            "{LFS_VERSION_PREFIX}\noid sha256:{}\nsize 4\n",
            "A".repeat(64)
        );
        assert!(GitCore::parse_lfs_pointer(pointer.as_bytes()).is_err());
    }

    #[test]
    fn action_authorization_takes_precedence() {
        let core = core(Some("user"), Some("pass"));
        let action = action(
            "https://example.com/object",
            &[("Authorization", "Bearer action-token")],
        );
        let headers = core.action_headers(&action);
        assert_eq!(
            headers
                .get(header::AUTHORIZATION)
                .unwrap()
                .to_str()
                .unwrap(),
            "Bearer action-token"
        );
    }

    #[test]
    fn repository_auth_is_same_origin_only() {
        let core = core(Some("user"), Some("pass"));
        let same = action("https://example.com/object", &[]);
        assert!(
            core.action_headers(&same)
                .contains_key(header::AUTHORIZATION)
        );

        let cross = action("https://cdn.example.net/object", &[]);
        assert!(
            !core
                .action_headers(&cross)
                .contains_key(header::AUTHORIZATION)
        );
    }

    #[test]
    fn missing_expiry_uses_cap() {
        let now = Instant::now();
        let reuse_until = compute_reuse_until(None, None, now, SystemTime::now());
        assert_eq!(reuse_until.duration_since(now), LFS_ACTION_REUSE_CAP);
    }

    #[test]
    fn advertised_expiry_is_capped_and_margined() {
        let now = Instant::now();
        let now_system = SystemTime::now();
        let reuse_until = compute_reuse_until(Some(120), None, now, now_system);
        assert_eq!(reuse_until.duration_since(now), LFS_ACTION_REUSE_CAP);

        let reuse_until = compute_reuse_until(Some(30), None, now, now_system);
        assert_eq!(reuse_until.duration_since(now), Duration::from_secs(25));
    }

    #[test]
    fn advertised_expires_at_can_be_shorter() {
        let now = Instant::now();
        let now_system: SystemTime = Timestamp::from_str("2030-01-01T00:00:00Z").unwrap().into();
        let reuse_until =
            compute_reuse_until(Some(120), Some("2030-01-01T00:00:20Z"), now, now_system);
        assert_eq!(reuse_until.duration_since(now), Duration::from_secs(15));
    }
}
