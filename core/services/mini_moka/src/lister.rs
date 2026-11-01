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

use super::core::MiniMokaCore;

pub struct MiniMokaLister {
    root: String,
    entries: IntoIter<(String, Metadata)>,
}

impl MiniMokaLister {
    pub fn new(core: Arc<MiniMokaCore>, root: String, _path: String) -> Self {
        let entries: Vec<(String, Metadata)> = core
            .cache
            .iter()
            .map(|entry| (entry.key().to_string(), entry.value().metadata.clone()))
            .collect();

        Self {
            root,
            entries: entries.into_iter(),
        }
    }
}

impl oio::List for MiniMokaLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        match self.entries.next() {
            Some((key, metadata)) => {
                let rel_path = build_rel_path(&self.root, &key);

                Ok(Some(oio::Entry::new(&rel_path, metadata)))
            }
            None => Ok(None),
        }
    }
}
