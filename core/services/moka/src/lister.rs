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

use super::core::MokaCore;
use opendal_core::raw::oio;
use opendal_core::raw::*;
use opendal_core::*;

pub struct MokaLister {
    root: String,
    entries: IntoIter<(String, Metadata)>,
}

impl MokaLister {
    pub fn new(core: Arc<MokaCore>, root: String, _path: String) -> Self {
        let entries: Vec<(String, Metadata)> = core
            .cache
            .iter()
            .map(|(key, value)| (key.to_string(), value.metadata.clone()))
            .collect();

        Self {
            root,
            entries: entries.into_iter(),
        }
    }
}

impl oio::List for MokaLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        match self.entries.next() {
            Some((key, metadata)) => {
                let mut path = build_rel_path(&self.root, &key);
                if path.is_empty() {
                    path = "/".to_string();
                }

                Ok(Some(oio::Entry::new(&path, metadata)))
            }
            None => Ok(None),
        }
    }
}
