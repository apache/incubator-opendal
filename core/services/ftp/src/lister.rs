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

use std::str;
use std::str::FromStr;
use std::vec::IntoIter;

use suppaftp::list::File;

use opendal_core::raw::*;
use opendal_core::*;

pub struct FtpLister {
    path: String,
    file_iter: IntoIter<String>,
}

impl FtpLister {
    pub fn new(path: &str, files: Vec<String>) -> Self {
        Self {
            path: path.to_string(),
            file_iter: files.into_iter(),
        }
    }
}

impl oio::List for FtpLister {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        let de = match self.file_iter.next() {
            Some(file_str) => File::from_str(file_str.as_str()).map_err(|e| {
                Error::new(ErrorKind::Unexpected, "parse file from response").set_source(e)
            })?,
            None => return Ok(None),
        };

        let path = self.path.to_string() + de.name();

        let mut meta = if de.is_file() {
            MetadataBuilder::file(de.size() as u64)
        } else if de.is_directory() {
            MetadataBuilder::dir()
        } else {
            MetadataBuilder::unknown()
        };
        meta.last_modified(Timestamp::try_from(de.modified())?);

        let entry = if de.is_file() {
            oio::Entry::new(&path, meta.build())
        } else if de.is_directory() {
            oio::Entry::new(&format!("{}/", path), meta.build())
        } else {
            oio::Entry::new(&path, meta.build())
        };

        Ok(Some(entry))
    }
}
