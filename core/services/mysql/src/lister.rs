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

use opendal_core::MetadataBuilder;
use opendal_core::Result;
use opendal_core::raw::oio::{Entry, List};
use opendal_core::raw::{build_abs_path, build_rel_path};

use super::core::MysqlCore;

pub struct MysqlLister {
    root: String,
    entries: IntoIter<String>,
}

impl MysqlLister {
    pub async fn new(core: Arc<MysqlCore>, root: String, path: String) -> Result<Self> {
        let entries = core.list(&build_abs_path(&root, &path)).await?.into_iter();
        Ok(Self { root, entries })
    }
}

impl List for MysqlLister {
    async fn next(&mut self) -> Result<Option<Entry>> {
        let Some(key) = self.entries.next() else {
            return Ok(None);
        };

        let mut path = build_rel_path(&self.root, &key);
        if path.is_empty() {
            path = "/".to_string();
        }
        let meta = if path.ends_with('/') {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::unknown()
        };
        Ok(Some(Entry::new(&path, meta.build())))
    }
}

pub struct MysqlLazyLister {
    core: Arc<MysqlCore>,
    root: String,
    path: String,
    inner: Option<MysqlLister>,
}

impl MysqlLazyLister {
    pub fn new(core: Arc<MysqlCore>, root: String, path: String) -> Self {
        Self {
            core,
            root,
            path,
            inner: None,
        }
    }
}

impl List for MysqlLazyLister {
    async fn next(&mut self) -> Result<Option<Entry>> {
        if self.inner.is_none() {
            self.inner = Some(
                MysqlLister::new(self.core.clone(), self.root.clone(), self.path.clone()).await?,
            );
        }

        self.inner
            .as_mut()
            .expect("lister must be initialized")
            .next()
            .await
    }
}
