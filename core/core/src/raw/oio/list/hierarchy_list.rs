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

use std::collections::HashSet;

use crate::raw::*;
use crate::*;

/// ToHierarchyLister will convert a flat list to hierarchy by filter
/// not needed entries.
///
/// # Notes
///
/// ToHierarchyLister filter entries after fetch entries. So it's possible
/// to return an empty vec. It doesn't mean the all pages have been
/// returned.
///
/// Please keep calling next until we returned `Ok(None)`
pub struct HierarchyLister<P> {
    lister: P,
    path: String,
    visited: HashSet<String>,
    recursive: bool,
}

impl<P> HierarchyLister<P> {
    /// Create a new hierarchy lister
    pub fn new(lister: P, path: &str, recursive: bool) -> HierarchyLister<P> {
        let path = if path == "/" {
            "".to_string()
        } else {
            path.to_string()
        };

        HierarchyLister {
            lister,
            path,
            visited: HashSet::default(),
            recursive,
        }
    }

    /// ## NOTES
    ///
    /// We take `&mut Entry` here because we need to perform modification on entry in the case like
    /// listing "a/" with existing key `a/b/c`.
    ///
    /// In this case, we got a key `a/b/c`, but we should return `a/b/` instead to keep the hierarchy.
    fn keep_entry(&mut self, e: &mut oio::Entry) -> bool {
        // If path is not started with prefix, drop it.
        //
        // Ideally, it should never happen. But we just tolerate
        // this state.
        if !e.path().starts_with(&self.path) {
            return false;
        }

        // Don't return already visited path.
        if self.visited.contains(e.path()) {
            return false;
        }

        let prefix_len = self.path.len();

        let idx = if let Some(idx) = e.path()[prefix_len..].find('/') {
            idx + prefix_len + 1
        } else {
            // If there is no `/` in path, it's a normal file, we
            // can return it directly.
            return true;
        };

        // idx == path.len() means it's contain only one `/` at the
        // end of path.
        if idx == e.path().len() {
            if !self.visited.contains(e.path()) {
                self.visited.insert(e.path().to_string());
            }
            return true;
        }

        // If idx < path.len() means that are more levels to come.
        // We should check the first dir path.
        let has = {
            let path = &e.path()[..idx];
            self.visited.contains(path)
        };
        if !has {
            let path = {
                let path = &e.path()[..idx];
                path.to_string()
            };

            let mut builder = e.metadata().clone().into_builder();
            builder.set_dir();
            *e = oio::Entry::with(path.clone(), builder.build());
            self.visited.insert(path);

            return true;
        }

        false
    }
}

impl<P: oio::List> oio::List for HierarchyLister<P> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        loop {
            let mut entry = match self.lister.next().await? {
                Some(entry) => entry,
                None => return Ok(None),
            };

            if self.recursive {
                return Ok(Some(entry));
            }
            if self.keep_entry(&mut entry) {
                return Ok(Some(entry));
            }
        }
    }
}
