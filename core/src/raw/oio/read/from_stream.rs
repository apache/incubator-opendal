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

use crate::raw::oio::Read;
use crate::*;
use futures::Stream;
use futures::StreamExt;

pub fn from_stream<S>(stream: S) -> FromStream<S>
where
    S: Stream<Item = Result<Buffer>> + Send + Sync + Unpin + 'static,
{
    FromStream(stream)
}

pub struct FromStream<S>(S);

impl<S> Read for FromStream<S>
where
    S: Stream<Item = Result<Buffer>> + Send + Sync + Unpin + 'static,
{
    async fn read(&mut self) -> Result<Buffer> {
        match self.0.next().await {
            Some(Ok(buf)) => Ok(buf),
            Some(Err(err)) => Err(err),
            None => Ok(Buffer::new()),
        }
    }
}