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

use std::future::pending;
use std::sync::{Arc, Mutex};

use anyhow::Result as AnyhowResult;
use bytes::Bytes;
use od::raw::oio;
use od::raw::*;
use od::{
    Buffer, BytesRange, Capability, Error, ErrorKind, MetadataBuilder, OperationContext, Result,
};
use opendal as od;

use crate::Operator;
use crate::layer::LayerBuilder;

static RETRY_ATTEMPT: std::sync::LazyLock<Arc<Mutex<usize>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(0)));

pub fn retryable_attempt_count() -> usize {
    *RETRY_ATTEMPT.lock().unwrap()
}

fn reset_retryable_attempts() {
    *RETRY_ATTEMPT.lock().unwrap() = 0;
}

fn build_operator(
    layers: &LayerBuilder,
    service: impl Service + 'static,
) -> AnyhowResult<*mut Operator> {
    let op = od::Operator::from_parts(OperationContext::default(), Arc::new(service));
    let op = layers.apply(op);
    let op = crate::into_blocking_operator(op)?;
    Ok(Box::into_raw(Box::new(op)))
}

pub fn new_hanging_operator(layers: &LayerBuilder) -> AnyhowResult<*mut Operator> {
    build_operator(layers, HangingService)
}

pub fn new_retryable_operator(layers: &LayerBuilder) -> AnyhowResult<*mut Operator> {
    reset_retryable_attempts();
    build_operator(
        layers,
        RetryableService {
            attempt: RETRY_ATTEMPT.clone(),
        },
    )
}

#[derive(Debug, Clone, Default)]
struct HangingService;

impl Service for HangingService {
    type Reader = oio::StreamReader<HangingReader>;
    type Writer = ();
    type Lister = ();
    type Deleter = ();
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        ServiceInfo::with_scheme("mock-hanging")
    }

    fn capability(&self) -> Capability {
        Capability {
            read: true,
            stat: true,
            ..Default::default()
        }
    }

    async fn create_dir(
        &self,
        _: &OperationContext,
        _: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
        let metadata = MetadataBuilder::file(13);
        Ok(RpStat::new(metadata.build()))
    }

    fn read(&self, _: &OperationContext, _: &str, _: OpRead) -> Result<Self::Reader> {
        Ok(oio::StreamReader::new(HangingReader))
    }

    fn write(&self, _: &OperationContext, _: &str, _: OpWrite) -> Result<Self::Writer> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn delete(&self, _: &OperationContext) -> Result<Self::Deleter> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn list(&self, _: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn copy(&self, _: &OperationContext, _: &str, _: &str, _: OpCopy) -> Result<Self::Copier> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn rename(
        &self,
        _: &OperationContext,
        _: &str,
        _: &str,
        _: OpRename,
    ) -> Result<RpRename> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}

#[derive(Debug, Clone, Default)]
struct HangingReader;

impl oio::StreamRead for HangingReader {
    async fn open(&self, _: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        pending::<()>().await;
        unreachable!()
    }
}

#[derive(Debug, Clone)]
struct RetryableService {
    attempt: Arc<Mutex<usize>>,
}

impl Service for RetryableService {
    type Reader = oio::StreamReader<RetryableReader>;
    type Writer = ();
    type Lister = ();
    type Deleter = ();
    type Copier = ();
    type Composer = ();

    fn info(&self) -> ServiceInfo {
        ServiceInfo::with_scheme("mock-retryable")
    }

    fn capability(&self) -> Capability {
        Capability {
            read: true,
            stat: true,
            ..Default::default()
        }
    }

    async fn create_dir(
        &self,
        _: &OperationContext,
        _: &str,
        _: OpCreateDir,
    ) -> Result<RpCreateDir> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn stat(&self, _: &OperationContext, _: &str, _: OpStat) -> Result<RpStat> {
        let metadata = MetadataBuilder::file(13);
        Ok(RpStat::new(metadata.build()))
    }

    fn read(&self, _: &OperationContext, _: &str, _: OpRead) -> Result<Self::Reader> {
        Ok(oio::StreamReader::new(RetryableReader {
            attempt: self.attempt.clone(),
        }))
    }

    fn write(&self, _: &OperationContext, _: &str, _: OpWrite) -> Result<Self::Writer> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn delete(&self, _: &OperationContext) -> Result<Self::Deleter> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn list(&self, _: &OperationContext, _: &str, _: OpList) -> Result<Self::Lister> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    fn copy(&self, _: &OperationContext, _: &str, _: &str, _: OpCopy) -> Result<Self::Copier> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn rename(
        &self,
        _: &OperationContext,
        _: &str,
        _: &str,
        _: OpRename,
    ) -> Result<RpRename> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }

    async fn presign(&self, _: &OperationContext, _: &str, _: OpPresign) -> Result<RpPresign> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "operation is not supported",
        ))
    }
}

#[derive(Debug, Clone)]
struct RetryableReader {
    attempt: Arc<Mutex<usize>>,
}

impl oio::StreamRead for RetryableReader {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let metadata = MetadataBuilder::file(13);
        let rp = RpRead::new(metadata.build());
        Ok((
            rp,
            Box::new(RetryableReadStream {
                buf: Bytes::from_static(b"Hello, World!").into(),
                range,
                attempt: self.attempt.clone(),
            }) as Box<dyn oio::ReadStreamDyn>,
        ))
    }
}

#[derive(Debug, Clone)]
struct RetryableReadStream {
    buf: Buffer,
    range: BytesRange,
    attempt: Arc<Mutex<usize>>,
}

impl oio::ReadStream for RetryableReadStream {
    async fn read(&mut self) -> Result<Buffer> {
        let mut attempt = self.attempt.lock().unwrap();
        *attempt += 1;

        match *attempt {
            1 | 2 | 4 => Err(
                Error::new(ErrorKind::Unexpected, "retryable_error from reader").set_temporary(),
            ),
            3 | 5 => Ok(self.buf.slice(self.range.to_range_as_usize())),
            n => Err(Error::new(
                ErrorKind::Unexpected,
                format!("unexpected retryable attempt {n}"),
            )),
        }
    }
}
