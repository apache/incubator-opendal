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

use hdfs_native::file::FileReader;

use crate::raw::*;
use crate::services::hdfs_native::error::parse_hdfs_error;
use crate::*;

pub struct HdfsNativeReader {
    f: FileReader,
}

impl HdfsNativeReader {
    pub fn new(f: FileReader) -> Self {
        HdfsNativeReader { f }
    }
}

impl oio::Read for HdfsNativeReader {
    async fn read_at(&self, offset: u64, limit: usize) -> Result<Buffer> {
        // Perform the read operation using read_range
        let bytes = self
            .f
            .read_range(offset as usize, limit)
            .await
            .map_err(parse_hdfs_error)?;

        Ok(Buffer::from(bytes))
    }

    async fn read(&mut self) -> Result<Buffer> {
        todo!()
    }
}
