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
use std::collections::VecDeque;

use asyncband::mutex::Mutex;
use asyncband::singleflight::Group;

use super::core::normalize_dir_path;
use opendal_core::raw::*;
use opendal_core::*;

pub trait GdrivePathQueryer {
    async fn root(&self, ctx: &OperationContext) -> Result<String>;
    async fn query(
        &self,
        ctx: &OperationContext,
        parent_id: &str,
        name: &str,
    ) -> Result<Option<String>>;
    async fn create_dir(
        &self,
        ctx: &OperationContext,
        parent_id: &str,
        name: &str,
    ) -> Result<String>;
}

#[derive(Default)]
struct GdrivePathIndexState {
    entries: HashMap<String, String>,
}

/// A gdrive-local path index that owns all cache semantics for path -> id resolution.
///
/// The index keeps canonical directory paths (`dir/`) internally, supports exact upserts,
/// can invalidate subtrees, and can refresh ancestor chains before re-resolving stale paths.
pub struct GdrivePathIndex<Q: GdrivePathQueryer> {
    query: Q,
    state: Mutex<GdrivePathIndexState>,
    ensure_dirs: Group<(String, String), String>,
}

impl<Q: GdrivePathQueryer> GdrivePathIndex<Q> {
    pub fn new(query: Q) -> Self {
        Self {
            query,
            state: Mutex::new(GdrivePathIndexState::default()),
            ensure_dirs: Group::new(),
        }
    }

    async fn root_id(&self, ctx: &OperationContext) -> Result<String> {
        self.query.root(ctx).await
    }

    fn canonical_dir_path(path: &str) -> String {
        normalize_dir_path(path)
    }

    fn remove_path_candidates(entries: &mut HashMap<String, String>, path: &str) {
        entries.remove(path);

        let dir_path = Self::canonical_dir_path(path);
        if dir_path != path {
            entries.remove(&dir_path);
        }
    }

    fn lookup_cached_id(entries: &HashMap<String, String>, path: &str) -> Option<String> {
        if let Some(id) = entries.get(path) {
            return Some(id.clone());
        }

        let dir_path = Self::canonical_dir_path(path);
        if dir_path != path {
            return entries.get(&dir_path).cloned();
        }

        None
    }

    fn resolution_chain(path: &str) -> Vec<String> {
        let mut chain = VecDeque::new();
        let mut current = path;

        while current != "/" && !current.is_empty() {
            chain.push_front(current.to_string());
            current = get_parent(current);
        }

        chain.into_iter().collect()
    }

    async fn resolve_from_chain(
        &self,
        ctx: &OperationContext,
        chain: Vec<String>,
        start_index: usize,
        start_id: String,
    ) -> Result<Option<String>> {
        let mut current_id = start_id;

        for path in chain.into_iter().skip(start_index) {
            let name = get_basename(&path);
            current_id = match self.query.query(ctx, &current_id, name).await? {
                Some(id) => {
                    let cache_path = path;
                    if cache_path.ends_with('/') {
                        self.upsert_dir(&cache_path, &id).await;
                    } else {
                        self.upsert_file(&cache_path, &id).await;
                    }
                    id
                }
                None => return Ok(None),
            };
        }

        Ok(Some(current_id))
    }

    async fn lookup_resolve_plan(
        &self,
        ctx: &OperationContext,
        path: &str,
    ) -> Result<(Option<String>, Vec<String>)> {
        if path.is_empty() || path == "/" {
            return Ok((Some(self.root_id(ctx).await?), Vec::new()));
        }

        let chain = Self::resolution_chain(path);
        if chain.is_empty() {
            return Ok((Some(self.root_id(ctx).await?), Vec::new()));
        }

        let (start_id, start_index) = {
            let state = self.state.lock().await;
            let mut found = None;

            for idx in (0..chain.len()).rev() {
                if let Some(id) = Self::lookup_cached_id(&state.entries, &chain[idx]) {
                    found = Some((id, idx + 1));
                    break;
                }
            }

            found.unwrap_or((String::new(), 0))
        };

        if start_index == 0 {
            Ok((None, chain))
        } else {
            Ok((
                Some(start_id),
                chain.into_iter().skip(start_index).collect(),
            ))
        }
    }

    /// Get the id for the given path.
    pub async fn get(&self, ctx: &OperationContext, path: &str) -> Result<Option<String>> {
        if path.is_empty() || path == "/" {
            return self.root_id(ctx).await.map(Some);
        }

        if let Some(id) = {
            let state = self.state.lock().await;
            Self::lookup_cached_id(&state.entries, path)
        } {
            return Ok(Some(id));
        }

        let (start_id, missing_paths) = self.lookup_resolve_plan(ctx, path).await?;
        match start_id {
            Some(start_id) => {
                self.resolve_from_chain(ctx, missing_paths, 0, start_id)
                    .await
            }
            None => {
                let root_id = self.root_id(ctx).await?;
                self.resolve_from_chain(ctx, missing_paths, 0, root_id)
                    .await
            }
        }
    }

    /// Insert or replace a file mapping.
    pub async fn upsert_file(&self, path: &str, id: &str) {
        let mut state = self.state.lock().await;
        Self::remove_path_candidates(&mut state.entries, path);
        state.entries.insert(path.to_string(), id.to_string());
    }

    /// Insert or replace a directory mapping using canonical `dir/` storage.
    pub async fn upsert_dir(&self, path: &str, id: &str) {
        let dir_path = Self::canonical_dir_path(path);

        let mut state = self.state.lock().await;
        state.entries.remove(dir_path.trim_end_matches('/'));
        Self::remove_path_candidates(&mut state.entries, &dir_path);
        state.entries.insert(dir_path, id.to_string());
    }

    /// Remove a file mapping.
    pub async fn invalidate_file(&self, path: &str) {
        let mut state = self.state.lock().await;
        Self::remove_path_candidates(&mut state.entries, path);
    }

    /// Remove a directory mapping and all known descendants.
    pub async fn invalidate_dir(&self, path: &str) {
        let dir_path = Self::canonical_dir_path(path);

        let mut state = self.state.lock().await;
        let file_path = dir_path.trim_end_matches('/').to_string();
        state.entries.remove(&file_path);
        state
            .entries
            .retain(|entry_path, _| !entry_path.starts_with(&dir_path));
    }

    /// Invalidate the given path plus its ancestor chain before resolving it again.
    pub async fn refresh_path(&self, path: &str) {
        if path.is_empty() || path == "/" {
            return;
        }

        let mut current = path.to_string();
        while current != "/" && !current.is_empty() {
            let dir_path = Self::canonical_dir_path(&current);
            let mut state = self.state.lock().await;
            Self::remove_path_candidates(&mut state.entries, &current);
            Self::remove_path_candidates(&mut state.entries, &dir_path);
            drop(state);
            current = get_parent(&current).to_string();
        }
    }

    /// Ensure input dir exists.
    pub async fn ensure_dir(&self, ctx: &OperationContext, path: &str) -> Result<String> {
        let path = Self::canonical_dir_path(path);
        if path.is_empty() {
            return self.root_id(ctx).await;
        }

        let mut tmp = String::new();
        let mut parents = vec![];
        for component in path.split('/') {
            if component.is_empty() {
                continue;
            }

            tmp.push_str(component);
            tmp.push('/');
            parents.push(tmp.clone());
        }

        let mut refreshed = false;
        'retry: loop {
            let mut parent_id = self.root_id(ctx).await?;
            for parent in &parents {
                if let Some(value) = {
                    let state = self.state.lock().await;
                    Self::lookup_cached_id(&state.entries, parent)
                } {
                    parent_id = value;
                    continue;
                }

                let name = get_basename(parent);
                // A refreshed ancestor has a different ID and must not join work for its old ID.
                let result: Result<String> = self
                    .ensure_dirs
                    .try_work((parent_id.clone(), name.to_string()), async || {
                        if let Some(value) = {
                            let state = self.state.lock().await;
                            Self::lookup_cached_id(&state.entries, parent)
                        } {
                            return Ok(value);
                        }

                        let value = match self.query.query(ctx, &parent_id, name).await? {
                            Some(value) => value,
                            None => self.query.create_dir(ctx, &parent_id, name).await?,
                        };
                        // Publish before completing the flight so a later caller uses the cache.
                        self.upsert_dir(parent, &value).await;
                        Ok(value)
                    })
                    .await;
                parent_id = match result {
                    Ok(value) => value,
                    Err(err) if !refreshed && err.kind() == ErrorKind::NotFound => {
                        refreshed = true;
                        self.refresh_path(parent).await;
                        continue 'retry;
                    }
                    Err(err) => return Err(err),
                };
            }

            return Ok(parent_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::task::Context;
    use std::task::Waker;

    use asyncband::mutex::Mutex;
    use asyncband::semaphore::Semaphore;

    use super::*;

    #[derive(Clone)]
    struct TestQuery {
        root_id: String,
        entries: Arc<Mutex<HashMap<(String, String), String>>>,
        stale_parents: Arc<Mutex<HashSet<String>>>,
        create_count: Arc<AtomicUsize>,
        create_gate: Option<Arc<Semaphore>>,
    }

    impl TestQuery {
        fn new() -> Self {
            Self {
                root_id: "remote-root".to_string(),
                entries: Arc::new(Mutex::new(HashMap::new())),
                stale_parents: Arc::new(Mutex::new(HashSet::new())),
                create_count: Arc::new(AtomicUsize::new(0)),
                create_gate: None,
            }
        }

        async fn insert(&self, parent_id: &str, name: &str, id: &str) {
            self.entries
                .lock()
                .await
                .insert((parent_id.to_string(), name.to_string()), id.to_string());
        }

        async fn mark_stale_parent(&self, parent_id: &str) {
            self.stale_parents
                .lock()
                .await
                .insert(parent_id.to_string());
        }
    }

    impl GdrivePathQueryer for TestQuery {
        async fn root(&self, _: &OperationContext) -> Result<String> {
            Ok(self.root_id.clone())
        }

        async fn query(
            &self,
            _: &OperationContext,
            parent_id: &str,
            name: &str,
        ) -> Result<Option<String>> {
            if self.stale_parents.lock().await.contains(parent_id) {
                return Err(Error::new(ErrorKind::NotFound, "stale parent"));
            }
            Ok(self
                .entries
                .lock()
                .await
                .get(&(parent_id.to_string(), name.to_string()))
                .cloned())
        }

        async fn create_dir(
            &self,
            _: &OperationContext,
            parent_id: &str,
            name: &str,
        ) -> Result<String> {
            if self.stale_parents.lock().await.contains(parent_id) {
                return Err(Error::new(ErrorKind::NotFound, "stale parent"));
            }
            self.create_count.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = &self.create_gate {
                drop(gate.acquire(1).await);
            }
            let id = format!("{parent_id}:{name}");
            self.insert(parent_id, name, &id).await;
            Ok(id)
        }
    }

    #[tokio::test]
    async fn test_upsert_replaces_existing_path() {
        let index = GdrivePathIndex::new(TestQuery::new());
        let ctx = OperationContext::new();
        index.upsert_file("root/file", "old-id").await;
        index.upsert_file("root/file", "new-id").await;

        assert_eq!(
            index.get(&ctx, "root/file").await.unwrap().as_deref(),
            Some("new-id")
        );
    }

    #[tokio::test]
    async fn test_dir_upsert_replaces_file_mapping() {
        let index = GdrivePathIndex::new(TestQuery::new());
        let ctx = OperationContext::new();
        index.upsert_file("root/dir", "file-id").await;
        index.upsert_dir("root/dir", "dir-id").await;

        assert_eq!(
            index.get(&ctx, "root/dir").await.unwrap().as_deref(),
            Some("dir-id")
        );
        assert_eq!(
            index.get(&ctx, "root/dir/").await.unwrap().as_deref(),
            Some("dir-id")
        );
    }

    #[tokio::test]
    async fn test_subtree_invalidation_re_resolves_descendants() {
        let query = TestQuery::new();
        let index = GdrivePathIndex::new(query.clone());

        query.insert("remote-root", "root/", "svc-root").await;
        query.insert("svc-root", "dir/", "dir-old").await;
        query.insert("dir-old", "file", "file-old").await;
        let ctx = OperationContext::new();

        assert_eq!(
            index.get(&ctx, "root/dir/file").await.unwrap().as_deref(),
            Some("file-old")
        );

        query.insert("svc-root", "dir/", "dir-new").await;
        query.insert("dir-new", "file", "file-new").await;

        index.invalidate_dir("root/dir/").await;

        assert_eq!(
            index.get(&ctx, "root/dir/file").await.unwrap().as_deref(),
            Some("file-new")
        );
    }

    #[tokio::test]
    async fn test_ensure_dir_reuses_existing_directory() {
        let query = TestQuery::new();
        query.insert("remote-root", "root/", "svc-root").await;

        let index = GdrivePathIndex::new(query.clone());

        let ctx = OperationContext::new();
        let (first, second) = tokio::join!(
            index.ensure_dir(&ctx, "root/dir"),
            index.ensure_dir(&ctx, "root/dir")
        );

        assert_eq!(first.unwrap(), second.unwrap());
        assert_eq!(query.create_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_ensure_dir_coalesces_shared_ancestors() {
        let gate = Arc::new(Semaphore::new(0));
        let mut query = TestQuery::new();
        query.create_gate = Some(gate.clone());
        query.insert("remote-root", "root/", "svc-root").await;
        let index = GdrivePathIndex::new(query.clone());
        let ctx = OperationContext::new();
        let mut first = Box::pin(index.ensure_dir(&ctx, "root/shared/left"));
        let mut second = Box::pin(index.ensure_dir(&ctx, "root/shared/right"));
        let mut cx = Context::from_waker(Waker::noop());

        assert!(first.as_mut().poll(&mut cx).is_pending());
        assert!(second.as_mut().poll(&mut cx).is_pending());
        assert_eq!(query.create_count.load(Ordering::SeqCst), 1);

        gate.release(1);
        let (first, second) = tokio::join!(first, second);
        assert_eq!(first.unwrap(), "svc-root:shared/:left/");
        assert_eq!(second.unwrap(), "svc-root:shared/:right/");
        assert_eq!(query.create_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_ensure_dir_unrelated_paths_progress_concurrently() {
        for (first_path, second_path) in [("root/left", "root/right"), ("left/dir", "right/dir")] {
            let gate = Arc::new(Semaphore::new(0));
            let mut query = TestQuery::new();
            query.create_gate = Some(gate.clone());
            for name in ["root/", "left/", "right/"] {
                query.insert("remote-root", name, name).await;
            }
            let index = GdrivePathIndex::new(query.clone());
            let ctx = OperationContext::new();
            let mut first = Box::pin(index.ensure_dir(&ctx, first_path));
            let mut second = Box::pin(index.ensure_dir(&ctx, second_path));
            let mut cx = Context::from_waker(Waker::noop());

            assert!(first.as_mut().poll(&mut cx).is_pending());
            assert!(second.as_mut().poll(&mut cx).is_pending());
            assert_eq!(query.create_count.load(Ordering::SeqCst), 2);

            gate.release(1);
            let (first, second) = tokio::join!(first, second);
            assert_ne!(first.unwrap(), second.unwrap());
        }
    }

    #[tokio::test]
    async fn test_ensure_dir_retries_after_creator_is_cancelled() {
        let gate = Arc::new(Semaphore::new(0));
        let mut query = TestQuery::new();
        query.create_gate = Some(gate.clone());
        query.insert("remote-root", "root/", "svc-root").await;
        let index = GdrivePathIndex::new(query.clone());
        let ctx = OperationContext::new();
        let mut first = Box::pin(index.ensure_dir(&ctx, "root/dir"));
        let mut second = Box::pin(index.ensure_dir(&ctx, "root/dir"));
        let mut cx = Context::from_waker(Waker::noop());

        assert!(first.as_mut().poll(&mut cx).is_pending());
        assert!(second.as_mut().poll(&mut cx).is_pending());
        drop(first);
        assert!(second.as_mut().poll(&mut cx).is_pending());
        assert_eq!(query.create_count.load(Ordering::SeqCst), 2);

        gate.release(1);
        assert_eq!(second.await.unwrap(), "svc-root:dir/");
        assert_eq!(
            index.ensure_dir(&ctx, "root/dir").await.unwrap(),
            "svc-root:dir/"
        );
        assert_eq!(query.create_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_ensure_dir_refreshes_stale_ancestor_chain() {
        let query = TestQuery::new();
        query.insert("remote-root", "root/", "root-old").await;

        let index = GdrivePathIndex::new(query.clone());
        index.upsert_dir("root/", "root-old").await;
        let ctx = OperationContext::new();

        query.mark_stale_parent("root-old").await;
        query.insert("remote-root", "root/", "root-new").await;

        let dir_id = index.ensure_dir(&ctx, "root/dir").await.unwrap();

        assert_eq!(dir_id, "root-new:dir/");
        assert_eq!(query.create_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            index.get(&ctx, "root/dir").await.unwrap().as_deref(),
            Some("root-new:dir/")
        );
    }
}
