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

use std::cmp::min;
use std::io::SeekFrom;

use bytes::Bytes;
use futures::StreamExt;

use crate::raw::*;
use crate::*;

/// Convert given stream `futures::Stream<Item = Result<Bytes>>` into [`oio::Reader`].
pub fn into_read_from_stream<S>(stream: S) -> FromStreamReader<S> {
    FromStreamReader {
        inner: stream,
        buf: Bytes::new(),
    }
}

/// FromStreamReader will convert a `futures::Stream<Item = Result<Bytes>>` into `oio::Read`
pub struct FromStreamReader<S> {
    inner: S,
    buf: Bytes,
}

impl<S, T> oio::Read for FromStreamReader<S>
where
    S: futures::Stream<Item = Result<T>> + Send + Sync + Unpin + 'static,
    T: Into<Bytes>,
{
    async fn seek(&mut self, _: SeekFrom) -> Result<u64> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "FromStreamReader can't support operation",
        ))
    }

    async fn read(&mut self, limit: usize) -> Result<Bytes> {
        if self.buf.is_empty() {
            self.buf = match self.inner.next().await.transpose()? {
                Some(v) => v.into(),
                None => return Ok(Bytes::new()),
            };
        }

        let bs = self.buf.split_to(min(limit, self.buf.len()));
        Ok(bs)
    }
}
