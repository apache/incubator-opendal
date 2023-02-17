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

use crate::raw::build_rel_path;
use crate::Result;
use crate::{raw::output, ObjectMetadata, ObjectMode};
use async_trait::async_trait;

use super::list_response::Multistatus;

pub struct DirStream {
    root: String,
    path: String,
    size: usize,
    multistates: Multistatus,
}

impl DirStream {
    pub fn new(root: &str, path: &str, multistates: Multistatus, limit: Option<usize>) -> Self {
        Self {
            root: root.into(),
            path: path.into(),
            size: limit.unwrap_or(1000),
            multistates,
        }
    }
}

#[async_trait]
impl output::Page for DirStream {
    async fn next_page(&mut self) -> Result<Option<Vec<output::Entry>>> {
        let mut oes: Vec<output::Entry> = Vec::new();
        for _ in 0..self.size {
            if let Some(de) = self.multistates.response.pop() {
                let path = de.href.clone();
                let normalized_path = &if self.root != path {
                    build_rel_path(&self.root, &path)
                } else {
                    path
                };

                if normalized_path.eq(&self.path) {
                    // WebDav server may return the current path as an entry.
                    continue;
                }

                let entry = if de.propstat.prop.resourcetype.value
                    == Some(super::list_response::ResourceType::Collection)
                {
                    output::Entry::new(normalized_path, ObjectMetadata::new(ObjectMode::DIR))
                } else {
                    output::Entry::new(normalized_path, ObjectMetadata::new(ObjectMode::FILE))
                };
                oes.push(entry);
            }
        }

        Ok(if oes.is_empty() { None } else { Some(oes) })
    }
}
