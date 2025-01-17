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

use chrono::DateTime;
use hdfs_native::client::ListStatusIterator;

use crate::raw::oio::Entry;
use crate::raw::{build_rel_path, oio};
use crate::services::hdfs_native::error::parse_hdfs_error;
use crate::*;

pub struct HdfsNativeLister {
    root: String,
    lsi: ListStatusIterator,
}

impl HdfsNativeLister {
    pub fn new(root: &str, lsi: ListStatusIterator) -> Self {
        Self {
            root: root.to_string(),
            lsi,
        }
    }
}

impl oio::List for HdfsNativeLister {
    async fn next(&mut self) -> Result<Option<Entry>> {
        if let Some(de) = self
            .lsi
            .next()
            .await
            .transpose()
            .map_err(parse_hdfs_error)?
        {
            let path = build_rel_path(&self.root, &de.path);

            let entry = if !de.isdir {
                let odt = DateTime::from_timestamp(de.modification_time as i64, 0);

                let Some(dt) = odt else {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        &format!("Failure in extracting modified_time for {}", path),
                    ));
                };
                let meta = Metadata::new(EntryMode::FILE)
                    .with_content_length(de.length as u64)
                    .with_last_modified(dt);
                oio::Entry::new(&path, meta)
            } else if de.isdir {
                // Make sure we are returning the correct path.
                oio::Entry::new(&format!("{path}/"), Metadata::new(EntryMode::DIR))
            } else {
                oio::Entry::new(&path, Metadata::new(EntryMode::Unknown))
            };

            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }
}
