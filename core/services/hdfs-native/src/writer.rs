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

use hdfs_native::file::FileWriter;

use crate::core::parse_hdfs_error;
use opendal_core::raw::*;
use opendal_core::*;
pub struct HdfsNativeWriter {
    f: FileWriter,
    size: u64,
}

impl HdfsNativeWriter {
    pub fn new(f: FileWriter, initial_size: u64) -> Self {
        HdfsNativeWriter {
            f,
            size: initial_size,
        }
    }
}

impl oio::Write for HdfsNativeWriter {
    async fn write(&mut self, mut buf: Buffer) -> Result<()> {
        let len = buf.len() as u64;

        for bs in buf.by_ref() {
            self.f.write_bytes(bs).await.map_err(parse_hdfs_error)?;
        }

        self.size += len;
        Ok(())
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.f.close().await.map_err(parse_hdfs_error)?;

        Ok({
            let mut metadata = MetadataBuilder::unknown();
            metadata.set_file(self.size);
            metadata.build()
        })
    }

    async fn abort(&mut self) -> Result<()> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "HdfsNativeWriter doesn't support abort",
        ))
    }
}
