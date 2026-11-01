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
use std::vec::IntoIter;

use opendal_core::raw::*;
use opendal_core::*;

use super::core::DashmapCore;

pub struct DashmapLister {
    root: String,
    path: String,
    iter: IntoIter<(String, Metadata)>,
}

impl DashmapLister {
    pub fn new(core: Arc<DashmapCore>, root: String, path: String) -> Self {
        let entries: Vec<_> = core
            .cache
            .iter()
            .map(|item| (item.key().clone(), item.value().metadata.clone()))
            .collect();
        let path = build_abs_path(&root, &path);

        Self {
            root,
            path,
            iter: entries.into_iter(),
        }
    }
}

impl oio::List for DashmapLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        for (key, metadata) in self.iter.by_ref() {
            if key.starts_with(&self.path) {
                let path = build_rel_path(&self.root, &key);
                let entry = oio::Entry::new(&path, metadata);
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }
}
