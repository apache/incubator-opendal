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

use std::fmt::Debug;

use async_trait::async_trait;

use crate::raw::*;
use crate::*;

/// CachePolicy allows user to specify the policy while caching.
#[async_trait]
pub trait CachePolicy: Send + Sync + Debug + 'static {
    /// The policy for reading cache.
    ///
    /// `on_read` will return a [`CacheReadEntryIterator`] which can iterate
    /// serval [`CacheReadEntry`]. Cache layer will take different operations
    /// as specified by [`CacheReadEntry`].
    ///
    /// # Notes
    ///
    /// It's implementor's abailty to make sure the returning entry is
    /// correct.
    async fn on_read(
        &self,
        path: &str,
        offset: u64,
        size: u64,
        total_size: u64,
    ) -> CacheReadEntryIterator;

    /// The policy for updating cache.
    ///
    /// `on_update` will return a [`CacheUpdateEntryIterator`] which can
    /// iterate serval [`CacheUpdateEntry`]. Cache layer will take different
    /// operations as specified by [`CacheUpdateEntry`].
    ///
    /// # Notes
    ///
    /// It's implementor's abailty to make sure the returning entry is
    /// correct.
    ///
    /// on_update will be called on `create`, `write` and `delete`.
    async fn on_update(&self, path: &str, op: Operation) -> CacheUpdateEntryIterator;
}

#[derive(Debug)]
pub struct DefaultCachePolicy;

#[async_trait]
impl CachePolicy for DefaultCachePolicy {
    async fn on_read(&self, path: &str, offset: u64, size: u64, _: u64) -> CacheReadEntryIterator {
        let br: BytesRange = (offset..offset + size).into();

        Box::new(
            vec![CacheReadEntry {
                cache_path: path.to_string(),

                read_cache: true,
                cache_read_range: br,
                inner_read_range: br,

                fill_method: CacheFillMethod::Async,
                cache_fill_range: br,
            }]
            .into_iter(),
        )
    }

    async fn on_update(&self, path: &str, _: Operation) -> CacheUpdateEntryIterator {
        Box::new(
            vec![CacheUpdateEntry {
                cache_path: path.to_string(),

                update_method: CacheUpdateMethod::Delete,
            }]
            .into_iter(),
        )
    }
}

/// CacheFillMethod specify the cache fill method while cache missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheFillMethod {
    /// Don't fill cache.
    ///
    /// Return data from inner directly without fill into the cache.
    Skip,
    /// Fill cache in sync way.
    ///
    /// Write data into cache first and than read from cache.
    Sync,
    /// Fill cache in async way.
    ///
    /// Spawn an async task to runtime and return data directly.
    Async,
}

/// CacheReadEntryIterator is a boxed iterator for [`CacheReadEntry`].
pub type CacheReadEntryIterator = Box<dyn Iterator<Item = CacheReadEntry> + Send>;

/// CacheReadEntry indicates the operations that cache layer needs to take.
///
/// # TODO
///
/// Add debug_assert to make sure:
///
/// - cache_read_range.size() == inner_read_range.size()
/// - cache_fill_range contains inner_read_range ?
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheReadEntry {
    /// cache_path is the path that we need to read or fill.
    pub cache_path: String,

    /// read_method indicates that do we need to read from cache.
    pub read_cache: bool,
    /// the range to read from cache file if we decide to read cache.
    pub cache_read_range: BytesRange,
    /// the range to read from inner file if we decide to skip cache
    /// or cache missed.
    pub inner_read_range: BytesRange,

    /// fill_method indicates that how we will fill the cache.
    pub fill_method: CacheFillMethod,
    /// the range to read from inner file to fill the cache.
    pub cache_fill_range: BytesRange,
}

impl CacheReadEntry {
    /// Build an OpRead from cache read range.
    pub fn cache_read_op(&self) -> OpRead {
        OpRead::new().with_range(self.cache_read_range)
    }

    /// Build an OpRead from inner read range.
    pub fn inner_read_op(&self) -> OpRead {
        OpRead::new().with_range(self.inner_read_range)
    }

    /// The size for cache fill.
    pub fn inner_read_size(&self) -> u64 {
        self.inner_read_range.size().expect("size must be valid")
    }

    /// Build an OpRead from cache fill range.
    pub fn cache_fill_op(&self) -> OpRead {
        OpRead::new().with_range(self.cache_fill_range)
    }

    /// The size for cache fill.
    pub fn cache_fill_size(&self) -> u64 {
        self.cache_fill_range.size().expect("size must be valid")
    }
}

/// CacheUpdateEntryIterator is a boxed iterator for [`CacheUpdateEntry`].
pub type CacheUpdateEntryIterator = Box<dyn Iterator<Item = CacheUpdateEntry> + Send>;

#[derive(Debug, Clone, PartialEq, Eq)]
/// CacheUpdateEntry indicates the operations that cache layer needs to take.
pub struct CacheUpdateEntry {
    /// cache_path is the path that we need to read or fill.
    pub cache_path: String,

    /// update_method indicates that do we need to update the cache.
    pub update_method: CacheUpdateMethod,
}

/// CacheUpdateMethod specify the cache update method while inner files changed.
///
/// # Notes
///
/// We could add new method in the future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheUpdateMethod {
    /// Don't do anything on cache.
    ///
    /// Level the cache AS-IS until they cleaned by service itself.
    Skip,
    /// Delete the cache path.
    Delete,
}
