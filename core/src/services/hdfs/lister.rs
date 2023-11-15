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

use async_trait::async_trait;
use std::task::{Context, Poll};

use crate::raw::*;
use crate::EntryMode;
use crate::Metadata;
use crate::Result;

pub struct HdfsLister {
    root: String,

    size: usize,
    rd: hdrs::Readdir,
}

impl HdfsLister {
    pub fn new(root: &str, rd: hdrs::Readdir, limit: Option<usize>) -> Self {
        Self {
            root: root.to_string(),

            size: limit.unwrap_or(1000),
            rd,
        }
    }
}

#[async_trait]
impl oio::List for HdfsLister {
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Result<Option<oio::Entry>>> {
        let de = match self.rd.next() {
            Some(de) => de,
            None => return Poll::Ready(Ok(None)),
        };

        let path = build_rel_path(&self.root, de.path());

        let entry = if de.is_file() {
            let meta = Metadata::new(EntryMode::FILE)
                .with_content_length(de.len())
                .with_last_modified(de.modified().into());
            oio::Entry::new(&path, meta)
        } else if de.is_dir() {
            // Make sure we are returning the correct path.
            oio::Entry::new(&format!("{path}/"), Metadata::new(EntryMode::DIR))
        } else {
            oio::Entry::new(&path, Metadata::new(EntryMode::Unknown))
        };

        Poll::Ready(Ok(Some(entry)))
    }
}

impl oio::BlockingList for HdfsLister {
    fn next(&mut self) -> Result<Option<oio::Entry>> {
        let de = match self.rd.next() {
            Some(de) => de,
            None => return Ok(None),
        };

        let path = build_rel_path(&self.root, de.path());

        let entry = if de.is_file() {
            let meta = Metadata::new(EntryMode::FILE)
                .with_content_length(de.len())
                .with_last_modified(de.modified().into());
            oio::Entry::new(&path, meta)
        } else if de.is_dir() {
            // Make sure we are returning the correct path.
            oio::Entry::new(&format!("{path}/"), Metadata::new(EntryMode::DIR))
        } else {
            oio::Entry::new(&path, Metadata::new(EntryMode::Unknown))
        };

        Ok(Some(entry))
    }
}
