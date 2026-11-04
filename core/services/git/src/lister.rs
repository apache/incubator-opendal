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

use opendal_core::raw::*;
use opendal_core::*;

use super::core::GitCore;
use super::core::GitEntry;
use super::core::join_error;

/// Lister returned by the Git service.
pub struct GitLister {
    core: Arc<GitCore>,
    base_path: String,
    entries: Option<Vec<(String, GitEntry)>>,
    index: usize,
}

impl GitLister {
    pub(super) fn new(core: Arc<GitCore>, base_path: String) -> Self {
        Self {
            core,
            base_path,
            entries: None,
            index: 0,
        }
    }
}

impl oio::List for GitLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        if self.entries.is_none() {
            let core = self.core.clone();
            let path = self.base_path.clone();
            let entries = tokio::task::spawn_blocking(move || core.list_dir_blocking(&path))
                .await
                .map_err(join_error)??;
            self.entries = Some(entries);
        }

        let entries = self
            .entries
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::Unexpected, "git lister not initialized"))?;
        if self.index >= entries.len() {
            return Ok(None);
        }

        let (name, entry) = &entries[self.index];
        self.index += 1;

        let full_path = if self.base_path == "/" {
            name.clone()
        } else {
            format!("{}/{}", self.base_path.trim_end_matches('/'), name)
        };
        let path = if entry.is_dir && !full_path.ends_with('/') {
            format!("{full_path}/")
        } else {
            full_path
        };

        let mut builder = if entry.is_dir {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::file(entry.size)
        };
        if let Ok(timestamp) = Timestamp::from_second(entry.last_modified) {
            builder.last_modified(timestamp);
        }
        if let Some(etag) = &entry.etag {
            builder.etag(etag);
        }

        Ok(Some(oio::Entry::new(&path, builder.build())))
    }
}
