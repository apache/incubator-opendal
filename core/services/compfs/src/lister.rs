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

use std::fs::ReadDir;
use std::path::Path;
use std::sync::Arc;

use super::core::CompfsCore;
use opendal_core::raw::*;
use opendal_core::*;

#[derive(Debug)]
pub struct CompfsLister {
    core: Arc<CompfsCore>,
    root: Option<String>,
    read_dir: Option<ReadDir>,
}

impl CompfsLister {
    pub(super) fn new(core: Arc<CompfsCore>, root: &Path, read_dir: ReadDir) -> Self {
        let root = normalize(root, &core.root);
        Self {
            core,
            root: Some(root),
            read_dir: Some(read_dir),
        }
    }
}

fn normalize(path: &Path, root: &Path) -> String {
    normalize_path(
        &path
            .strip_prefix(root)
            .expect("cannot fail because the prefix is iterated")
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

fn next_entry(read_dir: &mut ReadDir, root: &Path) -> std::io::Result<Option<oio::Entry>> {
    loop {
        let Some(entry) = read_dir.next().transpose()? else {
            return Ok(None);
        };
        let path = entry.path();
        let rel_path = normalize(&path, root);

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };

        let entry = if file_type.is_file() {
            let de_metadata = match entry.metadata() {
                Ok(de_metadata) => de_metadata,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            };
            oio::Entry::new(&rel_path, {
                let metadata = MetadataBuilder::file(de_metadata.len());
                metadata.build()
            })
        } else if file_type.is_dir() {
            oio::Entry::new(&format!("{rel_path}/"), MetadataBuilder::dir().build())
        } else {
            oio::Entry::new(&rel_path, MetadataBuilder::unknown().build())
        };

        return Ok(Some(entry));
    }
}

impl oio::List for CompfsLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        if let Some(root) = self.root.take() {
            return Ok(Some(oio::Entry::new(
                &format!("{root}/"),
                MetadataBuilder::dir().build(),
            )));
        }
        let Some(mut read_dir) = self.read_dir.take() else {
            return Ok(None);
        };
        let root = self.core.root.clone();
        let (entry, read_dir) = self
            .core
            .exec_blocking(move || {
                let entry = next_entry(&mut read_dir, &root).map_err(new_std_io_error);
                (entry, read_dir)
            })
            .await?;
        if !matches!(entry, Ok(None)) {
            self.read_dir = Some(read_dir);
        }
        entry
    }
}
