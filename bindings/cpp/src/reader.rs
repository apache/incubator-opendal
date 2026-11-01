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

use anyhow::Result;
use opendal as od;

use super::ffi;

pub struct Reader {
    reader: od::blocking::Reader,
    position: u64,
    content_length: u64,
}

impl Reader {
    pub fn new(reader: od::blocking::Reader, content_length: u64) -> Self {
        Self {
            reader,
            position: 0,
            content_length,
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.position >= self.content_length || buf.is_empty() {
            return Ok(0);
        }

        let remaining = self.content_length - self.position;
        let len = u64::try_from(buf.len())?.min(remaining);
        let n = self.read_at(&mut buf[..usize::try_from(len)?], self.position)?;
        self.position = self
            .position
            .checked_add(u64::try_from(n)?)
            .ok_or_else(|| anyhow::anyhow!("reader position overflow"))?;
        Ok(n)
    }

    pub fn read_at(&self, mut buf: &mut [u8], offset: u64) -> Result<usize> {
        let len = u64::try_from(buf.len())?;
        let end = offset
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("read range end overflow"))?;
        Ok(self.reader.read_into(&mut buf, offset..end)?)
    }

    pub fn seek(&mut self, offset: i64, dir: ffi::SeekFrom) -> Result<u64> {
        let base = match dir {
            ffi::SeekFrom::Start => 0,
            ffi::SeekFrom::Current => i128::from(self.position),
            ffi::SeekFrom::End => i128::from(self.content_length),
            _ => return Err(anyhow::anyhow!("invalid seek dir")),
        };
        let pos = base + i128::from(offset);
        if pos < 0 {
            return Err(anyhow::anyhow!("invalid seek to negative position"));
        }

        self.position = u64::try_from(pos)?;
        Ok(self.position)
    }
}
