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

use std::sync::Arc;

use bytes::BufMut;

use super::core::HuggingfaceCore;
use crate::raw::*;

pub struct HuggingfaceReader {
    core: Arc<HuggingfaceCore>,

    path: String,
    op: OpRead,
}

impl HuggingfaceReader {
    pub fn new(core: Arc<HuggingfaceCore>, path: &str, op: OpRead) -> Self {
        HuggingfaceReader {
            core,
            path: path.to_string(),
            op,
        }
    }
}

impl oio::Read for HuggingfaceReader {
    async fn read_at(
        &self,
        mut buf: oio::WritableBuf,
        offset: u64,
    ) -> (oio::WritableBuf, Result<usize>) {
        let range = BytesRange::new(offset, Some(buf.remaining_mut() as u64));

        self.core.hf_resolve(&self.path, range, &self.op, buf).await
    }
}
