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

use opendal_core::raw::*;
use opendal_core::*;

use super::core::parse_storage_error;

pub struct RedbLister {
    root: String,
    prefix: String,
    range: Option<redb::Range<'static, &'static str, &'static [u8]>>,
}

impl RedbLister {
    pub fn new(
        root: String,
        prefix: String,
        range: redb::Range<'static, &'static str, &'static [u8]>,
    ) -> Self {
        Self {
            root,
            prefix,
            range: Some(range),
        }
    }
}

impl oio::List for RedbLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        let item = match self.range.as_mut() {
            Some(range) => range.next(),
            None => return Ok(None),
        };
        let Some(item) = item else {
            self.range = None;
            return Ok(None);
        };
        let (key, value) = item.map_err(parse_storage_error)?;
        let key = key.value();
        if !key.starts_with(&self.prefix) {
            self.range = None;
            return Ok(None);
        }

        let mut path = build_rel_path(&self.root, key);
        if path.is_empty() {
            path = "/".to_string();
        }
        let metadata = if path.ends_with('/') {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::file(value.value().len() as u64)
        };
        Ok(Some(oio::Entry::new(&path, metadata.build())))
    }
}
