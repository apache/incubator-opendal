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

use opendal_core::raw::*;
use opendal_core::*;

use crate::core::{ErrorContext, GcsGrpcCore, parse_generation, parse_status};
use crate::generated::google::storage::v2::{ReadObjectRequest, ReadObjectResponse};

pub(super) struct GcsGrpcReader {
    core: Arc<GcsGrpcCore>,
    ctx: OperationContext,
    path: String,
    args: OpRead,
}

impl GcsGrpcReader {
    pub(super) fn new(
        core: Arc<GcsGrpcCore>,
        ctx: OperationContext,
        path: &str,
        args: OpRead,
    ) -> Self {
        Self {
            core,
            ctx,
            path: path.to_string(),
            args,
        }
    }
}

impl oio::Read for GcsGrpcReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let object = self.core.object_name(&self.path);
        let (read_offset, read_limit) = parse_read_range(range)?;
        let request = ReadObjectRequest {
            bucket: self.core.bucket_resource(),
            object,
            generation: parse_generation(self.args.version())?,
            read_offset,
            read_limit,
        };
        let request = self
            .core
            .request(
                &self.ctx,
                request,
                &[("bucket", &self.core.bucket_resource())],
            )
            .await?;
        let response = self
            .core
            .client()
            .read_object(request)
            .await
            .map_err(|status| {
                parse_status(ErrorContext::new(ServiceOperation("ReadObject")), status)
            })?;
        Ok((
            RpRead::default(),
            Box::new(GcsGrpcReadStream {
                stream: response.into_inner(),
                done: false,
                error_context: ErrorContext::new(ServiceOperation("ReadObject")),
            }),
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let (rp, mut stream) = self.open(range).await?;
        Ok((rp, stream.read_all_dyn().await?))
    }
}

fn parse_read_range(range: BytesRange) -> Result<(i64, i64)> {
    let convert = |value: u64| {
        i64::try_from(value).map_err(|err| {
            Error::new(ErrorKind::RangeNotSatisfied, "GCS gRPC range exceeds i64").set_source(err)
        })
    };
    if range.is_suffix() {
        Ok((-convert(range.size().unwrap_or_default())?, 0))
    } else {
        Ok((
            convert(range.offset())?,
            convert(range.size().unwrap_or_default())?,
        ))
    }
}

struct GcsGrpcReadStream {
    stream: tonic::Streaming<ReadObjectResponse>,
    done: bool,
    error_context: ErrorContext,
}

impl oio::ReadStream for GcsGrpcReadStream {
    async fn read(&mut self) -> Result<Buffer> {
        if self.done {
            return Ok(Buffer::new());
        }
        loop {
            match self
                .stream
                .message()
                .await
                .map_err(|status| parse_status(self.error_context, status))?
            {
                Some(response) => {
                    if let Some(data) = response.checksummed_data
                        && !data.content.is_empty()
                    {
                        return Ok(Buffer::from(data.content));
                    }
                }
                None => {
                    self.done = true;
                    return Ok(Buffer::new());
                }
            }
        }
    }
}
