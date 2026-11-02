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

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

use asyncband::mutex::Mutex;
use http::StatusCode;

use super::core::GdriveFile;
use super::core::GdriveFileList;
use super::core::GdriveRecentPathState;
use super::core::parse_error;
use super::core::{ErrorContext, GdriveCore};
use opendal_core::raw::*;
use opendal_core::*;

pub struct GdriveLister {
    path: String,
    core: Arc<GdriveCore>,
    ctx: OperationContext,
    emitted_paths: Mutex<HashSet<String>>,
    recent_entries_loaded: Mutex<bool>,
}

impl GdriveLister {
    pub fn new(path: String, core: Arc<GdriveCore>, ctx: OperationContext) -> Self {
        Self {
            path,
            core,
            ctx,
            emitted_paths: Mutex::default(),
            recent_entries_loaded: Mutex::default(),
        }
    }

    async fn push_entry(
        &self,
        ctx: &mut oio::PageContext,
        path: String,
        metadata: Metadata,
    ) -> Result<()> {
        let mut emitted_paths = self.emitted_paths.lock().await;
        if emitted_paths.insert(path.clone()) {
            ctx.entries.push_back(oio::Entry::new(&path, metadata));
        }
        Ok(())
    }

    async fn apply_recent_entry(
        &self,
        ctx: &mut oio::PageContext,
        abs_path: String,
        metadata: Metadata,
    ) -> Result<()> {
        match self.core.recent_entry_for_path(&abs_path).await {
            GdriveRecentPathState::Present(recent_metadata) => {
                let path = build_rel_path(&self.core.root, &abs_path);
                self.push_entry(ctx, path, *recent_metadata).await
            }
            GdriveRecentPathState::Deleted => Ok(()),
            GdriveRecentPathState::Missing => {
                let path = build_rel_path(&self.core.root, &abs_path);
                self.push_entry(ctx, path, metadata).await
            }
        }
    }

    async fn inject_recent_entries(&self, ctx: &mut oio::PageContext) -> Result<()> {
        let mut recent_entries_loaded = self.recent_entries_loaded.lock().await;
        if *recent_entries_loaded {
            return Ok(());
        }
        *recent_entries_loaded = true;

        for (abs_path, metadata) in self.core.recent_entries_for_list(&self.path, false).await {
            let path = build_rel_path(&self.core.root, &abs_path);
            self.push_entry(ctx, path, metadata).await?;
        }

        Ok(())
    }
}

fn metadata_from_gdrive_file(file: &GdriveFile) -> Result<Metadata> {
    let mut metadata =
        if file.mime_type.as_str() == "application/vnd.google-apps.folder" {
            MetadataBuilder::dir()
        } else if let Some(size) = &file.size {
            MetadataBuilder::file(size.parse::<u64>().map_err(|e| {
                Error::new(ErrorKind::Unexpected, "parse content length").set_source(e)
            })?)
        } else {
            MetadataBuilder::unknown()
        };
    metadata.content_type(file.mime_type.clone());
    if let Some(modified_time) = &file.modified_time {
        metadata.last_modified(modified_time.parse::<Timestamp>().map_err(|e| {
            Error::new(ErrorKind::Unexpected, "parse last modified time").set_source(e)
        })?);
    }

    Ok(metadata.build())
}

impl oio::PageList for GdriveLister {
    async fn next_page(&self, ctx: &mut oio::PageContext) -> Result<()> {
        if let GdriveRecentPathState::Deleted = self.core.recent_entry_for_path(&self.path).await {
            ctx.done = true;
            return Ok(());
        }

        let file_id = match self.core.resolve_path(&self.ctx, &self.path).await? {
            Some(file_id) => Some(file_id),
            None => {
                self.core
                    .resolve_path_after_refresh(&self.ctx, &self.path)
                    .await?
            }
        };

        let file_id = match file_id {
            Some(file_id) => file_id,
            None => {
                ctx.done = true;
                return Ok(());
            }
        };

        let is_dir_path = self.path.is_empty() || self.path.ends_with('/');

        // Prefix listing: only return `path` itself if it exists.
        //
        // This matches OpenDAL's list semantics:
        // - `list("file")` returns `file`
        // - `list("dir")` returns `dir/` (but does NOT list its children)
        // - list children requires a trailing slash, like `list("dir/")`
        if !is_dir_path {
            let mut resp = self.core.gdrive_stat_by_id(&self.ctx, &file_id).await?;
            if resp.status() == StatusCode::NOT_FOUND {
                let file_id = match self
                    .core
                    .resolve_path_after_refresh(&self.ctx, &self.path)
                    .await?
                {
                    Some(file_id) => file_id,
                    None => {
                        ctx.done = true;
                        return Ok(());
                    }
                };

                resp = self.core.gdrive_stat_by_id(&self.ctx, &file_id).await?;
            }
            if resp.status() != StatusCode::OK {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("GetFile")),
                    resp,
                ));
            }

            let bytes = resp.into_body().to_bytes();
            let file =
                serde_json::from_slice::<GdriveFile>(&bytes).map_err(new_json_deserialize_error)?;
            let is_dir = file.mime_type.as_str() == "application/vnd.google-apps.folder";

            let abs_path = if is_dir {
                format!("{}/", self.path)
            } else {
                self.path.clone()
            };
            let metadata = metadata_from_gdrive_file(&file)?;

            self.apply_recent_entry(ctx, abs_path, metadata).await?;
            ctx.done = true;
            return Ok(());
        }

        let mut resp = self
            .core
            .gdrive_list(&self.ctx, file_id.as_str(), 1000, &ctx.token)
            .await?;

        if resp.status() == StatusCode::NOT_FOUND {
            self.core.refresh_dir_path(&self.path).await;
            let file_id = match self.core.resolve_path(&self.ctx, &self.path).await? {
                Some(file_id) => file_id,
                None => {
                    ctx.done = true;
                    return Ok(());
                }
            };
            resp = self
                .core
                .gdrive_list(&self.ctx, file_id.as_str(), 1000, &ctx.token)
                .await?;
        }

        let bytes = match resp.status() {
            StatusCode::OK => resp.into_body().to_bytes(),
            _ => {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("ListFiles")),
                    resp,
                ));
            }
        };

        // Google Drive returns an empty response when attempting to list a non-existent directory.
        if bytes.is_empty() {
            ctx.done = true;
            return Ok(());
        }

        // Include the current directory itself when handling the first page of the listing.
        if ctx.token.is_empty() && !ctx.done {
            let path = build_rel_path(&self.core.root, &self.path);
            self.push_entry(ctx, path, MetadataBuilder::dir().build())
                .await?;
            self.inject_recent_entries(ctx).await?;
        }

        let decoded_response =
            serde_json::from_slice::<GdriveFileList>(&bytes).map_err(new_json_deserialize_error)?;

        if let Some(next_page_token) = decoded_response.next_page_token {
            ctx.token = next_page_token;
        } else {
            ctx.done = true;
        }

        for mut file in decoded_response.files {
            if file.mime_type.as_str() == "application/vnd.google-apps.folder"
                && !file.name.ends_with('/')
            {
                file.name += "/";
            }

            let path = format!("{}{}", self.path, file.name);
            let metadata = metadata_from_gdrive_file(&file)?;
            self.apply_recent_entry(ctx, path, metadata).await?;
        }

        Ok(())
    }
}

// ============================================================================
// GdriveFlatLister - Efficient recursive listing implementation
// ============================================================================

// GdriveFlatLister implements efficient recursive listing for Google Drive.
//
// Unlike the generic FlatLister which makes one API call per directory,
// this implementation uses batch queries with OR conditions to list
// multiple directories in a single API call (up to 50 parent IDs per query).
//
// This optimization reduces API calls from O(n) to O(n/50) where n is the
// number of directories, matching rclone's performance.

/// Maximum number of parent IDs to include in a single batch query.
/// Google Drive API has limits on query length, 50 is a safe value.
const BATCH_SIZE: usize = 50;

/// Page size for API requests. Google Drive allows up to 1000.
const PAGE_SIZE: i32 = 1000;

pub struct GdriveFlatLister {
    core: Arc<GdriveCore>,
    ctx: OperationContext,
    root_path: String,
    prefix: Option<String>,

    /// Queue of directory IDs that still need to be listed
    pending_dirs: VecDeque<PendingDir>,

    /// Buffer of entries ready to be returned
    entry_buffer: VecDeque<oio::Entry>,

    /// Current batch of directory IDs being listed
    current_batch: Vec<String>,

    /// Map from directory ID to its absolute path (for reconstructing file paths)
    dir_id_to_path: HashMap<String, String>,

    /// Pagination token for current batch query
    page_token: String,

    /// Track emitted paths so recent local entries and later remote pages don't
    /// produce duplicates.
    emitted_paths: HashSet<String>,

    /// Whether we've already merged recent local entries for this listing.
    recent_entries_loaded: bool,

    /// Whether we've started processing
    started: bool,

    /// Whether listing is complete
    done: bool,
}

/// Represents a directory waiting to be listed
struct PendingDir {
    id: String,
    path: String,
}

impl GdriveFlatLister {
    pub fn new(root_path: String, core: Arc<GdriveCore>, ctx: OperationContext) -> Self {
        Self {
            core,
            ctx,
            root_path,
            prefix: None,
            pending_dirs: VecDeque::new(),
            entry_buffer: VecDeque::new(),
            current_batch: Vec::new(),
            dir_id_to_path: HashMap::new(),
            page_token: String::new(),
            emitted_paths: HashSet::new(),
            recent_entries_loaded: false,
            started: false,
            done: false,
        }
    }

    fn push_entry(&mut self, path: String, metadata: Metadata) {
        if self.emitted_paths.insert(path.clone()) {
            self.entry_buffer
                .push_back(oio::Entry::new(&path, metadata));
        }
    }

    fn apply_refreshed_batch(&mut self, refreshed_dirs: Vec<(String, String)>) -> bool {
        self.page_token.clear();
        self.current_batch.clear();

        for (dir_id, path) in refreshed_dirs {
            self.dir_id_to_path.insert(dir_id.clone(), path);
            self.current_batch.push(dir_id);
        }

        !self.current_batch.is_empty()
    }

    async fn apply_recent_entry(
        &mut self,
        abs_path: String,
        metadata: Metadata,
        recent_state: GdriveRecentPathState,
    ) -> Result<()> {
        match recent_state {
            GdriveRecentPathState::Present(recent_metadata) => {
                let rel_path = build_rel_path(&self.core.root, &abs_path);
                self.push_entry(rel_path, *recent_metadata);
            }
            GdriveRecentPathState::Deleted => {}
            GdriveRecentPathState::Missing => {
                let rel_path = build_rel_path(&self.core.root, &abs_path);
                self.push_entry(rel_path, metadata);
            }
        }

        Ok(())
    }

    async fn inject_recent_entries(&mut self) -> Result<()> {
        if self.recent_entries_loaded {
            return Ok(());
        }
        self.recent_entries_loaded = true;

        let scope_path = self.prefix.as_deref().unwrap_or(&self.root_path);
        for (abs_path, metadata) in self.core.recent_entries_for_list(scope_path, true).await {
            let rel_path = build_rel_path(&self.core.root, &abs_path);
            self.push_entry(rel_path, metadata);
        }

        Ok(())
    }

    /// Initialize the lister by resolving the root directory ID
    async fn initialize(&mut self) -> Result<()> {
        log::debug!(
            "GdriveFlatLister: initializing with root path: {:?}",
            self.root_path
        );

        if let GdriveRecentPathState::Deleted =
            self.core.recent_entry_for_path(&self.root_path).await
        {
            self.done = true;
            return Ok(());
        }

        let root_id = match self.core.resolve_path(&self.ctx, &self.root_path).await? {
            Some(id) => {
                log::debug!("GdriveFlatLister: root path resolved to ID: {:?}", id);
                id
            }
            None => match self
                .core
                .resolve_path_after_refresh(&self.ctx, &self.root_path)
                .await?
            {
                Some(id) => {
                    log::debug!("GdriveFlatLister: root path resolved to ID: {:?}", id);
                    id
                }
                None => {
                    log::debug!(
                        "GdriveFlatLister: root path not found: {:?}",
                        self.root_path
                    );
                    if self.root_path.ends_with('/') {
                        self.done = true;
                        return Ok(());
                    }

                    let prefix = self.root_path.clone();
                    self.prefix = Some(prefix.clone());

                    let mut parent_path = get_parent(&prefix).to_string();
                    if parent_path == "/" {
                        parent_path.clear();
                    }

                    let parent_id = match self.core.resolve_path(&self.ctx, &parent_path).await? {
                        Some(id) => id,
                        None => match self
                            .core
                            .resolve_path_after_refresh(&self.ctx, &parent_path)
                            .await?
                        {
                            Some(id) => id,
                            None => {
                                self.done = true;
                                return Ok(());
                            }
                        },
                    };

                    self.root_path = parent_path.clone();
                    self.pending_dirs.push_back(PendingDir {
                        id: parent_id,
                        path: parent_path,
                    });

                    self.inject_recent_entries().await?;
                    self.started = true;
                    return Ok(());
                }
            },
        };

        // Resolve the entry type for root path so we can handle:
        //
        // - `list("dir", recursive=true)` where `dir` is a folder but without trailing slash.
        // - `list("prefix", recursive=true)` where `prefix` points to a file or a file prefix.
        let mut resp = self.core.gdrive_stat_by_id(&self.ctx, &root_id).await?;
        if resp.status() == StatusCode::NOT_FOUND {
            if self.root_path.ends_with('/') {
                self.core.refresh_dir_path(&self.root_path).await;
            } else {
                self.core.refresh_path(&self.root_path).await;
            }

            let root_id = match self.core.resolve_path(&self.ctx, &self.root_path).await? {
                Some(id) => id,
                None => {
                    if self.root_path.ends_with('/') {
                        self.done = true;
                        return Ok(());
                    }

                    let prefix = self.root_path.clone();
                    self.prefix = Some(prefix.clone());

                    let mut parent_path = get_parent(&prefix).to_string();
                    if parent_path == "/" {
                        parent_path.clear();
                    }

                    let parent_id = match self.core.resolve_path(&self.ctx, &parent_path).await? {
                        Some(id) => id,
                        None => match self
                            .core
                            .resolve_path_after_refresh(&self.ctx, &parent_path)
                            .await?
                        {
                            Some(id) => id,
                            None => {
                                self.done = true;
                                return Ok(());
                            }
                        },
                    };

                    self.root_path = parent_path.clone();
                    self.pending_dirs.push_back(PendingDir {
                        id: parent_id,
                        path: parent_path,
                    });

                    self.inject_recent_entries().await?;
                    self.started = true;
                    return Ok(());
                }
            };

            resp = self.core.gdrive_stat_by_id(&self.ctx, &root_id).await?;
        }
        if resp.status() != StatusCode::OK {
            return Err(parse_error(
                ErrorContext::new(ServiceOperation("GetFile")),
                resp,
            ));
        }

        let bytes = resp.into_body().to_bytes();
        let root_file =
            serde_json::from_slice::<GdriveFile>(&bytes).map_err(new_json_deserialize_error)?;
        let root_is_dir = root_file.mime_type.as_str() == "application/vnd.google-apps.folder";

        if root_is_dir {
            // Directory paths must end with `/` (except the root which is represented by empty path).
            if !self.root_path.is_empty() && !self.root_path.ends_with('/') {
                self.root_path.push('/');
                self.core.cache_dir_id(&self.root_path, &root_id).await;
            }

            // Add the root directory entry first.
            let mut rel_path = build_rel_path(&self.core.root, &self.root_path);
            if !rel_path.is_empty() && !rel_path.ends_with('/') {
                rel_path.push('/');
            }
            self.push_entry(rel_path, MetadataBuilder::dir().build());

            // Queue the root directory for listing.
            self.pending_dirs.push_back(PendingDir {
                id: root_id.clone(),
                path: self.root_path.clone(),
            });
            self.dir_id_to_path.insert(root_id, self.root_path.clone());
        } else {
            // For file/prefix listing, start from its parent directory and filter results by prefix.
            let prefix = self.root_path.clone();
            self.prefix = Some(prefix.clone());

            let mut parent_path = get_parent(&prefix).to_string();
            if parent_path == "/" {
                parent_path.clear();
            }

            let parent_id = match self.core.resolve_path(&self.ctx, &parent_path).await? {
                Some(id) => id,
                None => match self
                    .core
                    .resolve_path_after_refresh(&self.ctx, &parent_path)
                    .await?
                {
                    Some(id) => id,
                    None => {
                        self.done = true;
                        return Ok(());
                    }
                },
            };

            self.root_path = parent_path.clone();
            self.pending_dirs.push_back(PendingDir {
                id: parent_id,
                path: parent_path,
            });
        }

        self.inject_recent_entries().await?;
        self.started = true;
        Ok(())
    }

    /// Fill the current batch with pending directory IDs
    fn fill_batch(&mut self) {
        self.current_batch.clear();
        while self.current_batch.len() < BATCH_SIZE {
            if let Some(dir) = self.pending_dirs.pop_front() {
                self.current_batch.push(dir.id.clone());
                self.dir_id_to_path.insert(dir.id, dir.path);
            } else {
                break;
            }
        }
    }

    /// Process a batch of directories using OR query
    async fn process_batch(&mut self) -> Result<()> {
        if self.current_batch.is_empty() {
            self.done = true;
            return Ok(());
        }

        log::debug!(
            "GdriveFlatLister: processing batch of {} directories: {:?}",
            self.current_batch.len(),
            self.current_batch
        );

        let mut resp = self
            .core
            .gdrive_list_batch(&self.ctx, &self.current_batch, PAGE_SIZE, &self.page_token)
            .await?;

        if resp.status() == StatusCode::NOT_FOUND {
            let current_batch = self.current_batch.clone();
            let mut refreshed_batch = Vec::new();
            for dir_id in current_batch {
                if let Some(path) = self.dir_id_to_path.get(&dir_id).cloned() {
                    self.core.refresh_dir_path(&path).await;
                    if let Some(new_id) = self.core.resolve_path(&self.ctx, &path).await? {
                        refreshed_batch.push((new_id, path));
                    }
                }
            }

            if !self.apply_refreshed_batch(refreshed_batch) {
                self.done = true;
                return Ok(());
            }

            resp = self
                .core
                .gdrive_list_batch(&self.ctx, &self.current_batch, PAGE_SIZE, &self.page_token)
                .await?;
        }

        let bytes = match resp.status() {
            StatusCode::OK => resp.into_body().to_bytes(),
            _ => {
                return Err(parse_error(
                    ErrorContext::new(ServiceOperation("ListFiles")),
                    resp,
                ));
            }
        };

        log::debug!("GdriveFlatLister: response size: {} bytes", bytes.len());

        if bytes.is_empty() {
            self.page_token.clear();
            self.fill_batch();
            return Ok(());
        }

        let decoded_response =
            serde_json::from_slice::<GdriveFileList>(&bytes).map_err(new_json_deserialize_error)?;

        log::debug!(
            "GdriveFlatLister: got {} files, next_page_token: {:?}",
            decoded_response.files.len(),
            decoded_response.next_page_token.is_some()
        );

        // Process files from the response FIRST (this may add new dirs to pending_dirs)
        for file in decoded_response.files {
            self.process_file(file).await?;
        }

        // Update pagination state AFTER processing files
        if let Some(next_page_token) = decoded_response.next_page_token {
            self.page_token = next_page_token;
        } else {
            self.page_token.clear();
            // Current batch is done, prepare next batch with newly discovered directories
            self.fill_batch();
        }

        Ok(())
    }

    /// Process a single file from the API response
    async fn process_file(&mut self, mut file: GdriveFile) -> Result<()> {
        let is_dir = file.mime_type.as_str() == "application/vnd.google-apps.folder";

        if is_dir && !file.name.ends_with('/') {
            file.name.push('/');
        }

        // Find the parent directory path.
        //
        // Google Drive files can have multiple parents. The batch query guarantees that at least
        // one parent is among the directories we are currently listing, but it may not be the
        // first one in the `parents` array. Always pick a parent that we can resolve.
        let parent_path = file
            .parents
            .iter()
            .find_map(|parent_id| self.dir_id_to_path.get(parent_id).cloned())
            .unwrap_or_else(|| self.root_path.clone());

        let abs_path = format!("{}{}", parent_path, file.name);
        let metadata = metadata_from_gdrive_file(&file)?;
        let recent_state = self.core.recent_entry_for_path(&abs_path).await;

        let matches_prefix = self
            .prefix
            .as_ref()
            .map(|prefix| abs_path.starts_with(prefix))
            .unwrap_or(true);

        if matches_prefix {
            self.apply_recent_entry(abs_path.clone(), metadata, recent_state.clone())
                .await?;
        }

        // If it's a directory, queue it for recursive listing if it can contain matching entries.
        let should_traverse = is_dir
            && !matches!(recent_state, GdriveRecentPathState::Deleted)
            && self
                .prefix
                .as_ref()
                .map(|prefix| abs_path.starts_with(prefix) || prefix.starts_with(&abs_path))
                .unwrap_or(true);

        if should_traverse {
            self.pending_dirs.push_back(PendingDir {
                id: file.id.clone(),
                path: abs_path.clone(),
            });
            self.dir_id_to_path.insert(file.id, abs_path);
        }

        Ok(())
    }
}

impl oio::List for GdriveFlatLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        // Initialize on first call
        if !self.started {
            self.initialize().await?;
            self.fill_batch();
        }

        loop {
            // Return buffered entries first
            if let Some(entry) = self.entry_buffer.pop_front() {
                return Ok(Some(entry));
            }

            // If we're done, return None
            if self.done {
                return Ok(None);
            }

            // Process more directories
            self.process_batch().await?;

            // Check if we got any entries or if we're done
            if self.entry_buffer.is_empty()
                && self.current_batch.is_empty()
                && self.pending_dirs.is_empty()
            {
                self.done = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use asyncband::mutex::Mutex;

    use super::*;
    use crate::core::GdrivePathQuery;
    use crate::core::GdriveRecentState;
    use crate::core::GdriveSigner;
    use crate::path_index::GdrivePathIndex;

    fn mock_gdrive_core() -> Arc<GdriveCore> {
        let info = ServiceInfo::new("gdrive", "", "");
        let signer = Arc::new(Mutex::new(GdriveSigner::new()));

        Arc::new(GdriveCore {
            info: info.clone(),
            capability: Capability::default(),
            root: "/".to_string(),
            drive_id: None,
            signer: signer.clone(),
            path_index: GdrivePathIndex::new(GdrivePathQuery::new(signer, None)),
            recent_entries: Mutex::new(GdriveRecentState::default()),
        })
    }

    fn mock_operation_context() -> OperationContext {
        OperationContext::new()
    }

    #[tokio::test]
    async fn test_process_file_skips_deleted_dirs_for_traversal() {
        let core = mock_gdrive_core();
        core.record_recent_delete("parent/dir/", EntryMode::DIR)
            .await;

        let mut lister =
            GdriveFlatLister::new("parent/".to_string(), core, mock_operation_context());
        lister
            .dir_id_to_path
            .insert("parent-id".to_string(), "parent/".to_string());

        lister
            .process_file(GdriveFile {
                mime_type: "application/vnd.google-apps.folder".to_string(),
                id: "dir-id".to_string(),
                name: "dir".to_string(),
                size: None,
                modified_time: None,
                parents: vec!["parent-id".to_string()],
            })
            .await
            .unwrap();

        assert!(lister.pending_dirs.is_empty());
        assert!(!lister.dir_id_to_path.contains_key("dir-id"));
        assert!(lister.entry_buffer.is_empty());
    }

    #[test]
    fn test_apply_refreshed_batch_clears_page_token() {
        let core = mock_gdrive_core();
        let mut lister =
            GdriveFlatLister::new("parent/".to_string(), core, mock_operation_context());
        lister.page_token = "stale-page-token".to_string();
        lister.current_batch = vec!["old-id".to_string()];
        lister
            .dir_id_to_path
            .insert("old-id".to_string(), "parent/".to_string());

        assert!(lister.apply_refreshed_batch(vec![("new-id".to_string(), "parent/".to_string(),)]));
        assert!(lister.page_token.is_empty());
        assert_eq!(lister.current_batch, vec!["new-id".to_string()]);
        assert_eq!(
            lister.dir_id_to_path.get("new-id").map(String::as_str),
            Some("parent/")
        );
    }
}
