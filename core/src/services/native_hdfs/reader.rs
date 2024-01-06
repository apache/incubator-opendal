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

use crate::raw::oio::{Read, Reader};
use bytes::Bytes;
use hdfs_native::file::FileReader;
use std::io::SeekFrom;
use std::task::{Context, Poll};

pub struct NativeHdfsReader {
    f: FileReader,
}

impl NativeHdfsReader {
    pub fn new(f: FileReader) -> Self {
        NativeHdfsReader { f }
    }
}

impl Read for NativeHdfsReader {
    fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<crate::Result<usize>> {
        todo!()
    }

    fn poll_seek(&mut self, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<crate::Result<u64>> {
        todo!()
    }

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<crate::Result<Bytes>>> {
        todo!()
    }
}
