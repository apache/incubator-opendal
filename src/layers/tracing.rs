// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::io::Result;
use std::sync::Arc;

use async_trait::async_trait;

use crate::ops::OpAbortMultipart;
use crate::ops::OpCompleteMultipart;
use crate::ops::OpCreate;
use crate::ops::OpCreateMultipart;
use crate::ops::OpDelete;
use crate::ops::OpList;
use crate::ops::OpPresign;
use crate::ops::OpRead;
use crate::ops::OpStat;
use crate::ops::OpWrite;
use crate::ops::OpWriteMultipart;
use crate::ops::PresignedRequest;
use crate::Accessor;
use crate::AccessorMetadata;
use crate::BytesReader;
use crate::DirStreamer;
use crate::Layer;
use crate::ObjectMetadata;

/// TracingLayer will add tracing for OpenDAL.
///
/// # Examples
///
/// ```
/// use anyhow::Result;
/// use opendal::layers::TracingLayer;
/// use opendal::Operator;
/// use opendal::Scheme;
///
/// let _ = Operator::from_env(Scheme::Fs)
///     .expect("must init")
///     .layer(TracingLayer);
/// ```
pub struct TracingLayer;

impl Layer for TracingLayer {
    fn layer(&self, inner: Arc<dyn Accessor>) -> Arc<dyn Accessor> {
        Arc::new(TracingAccessor { inner })
    }
}

#[derive(Debug)]
struct TracingAccessor {
    inner: Arc<dyn Accessor>,
}

#[async_trait]
impl Accessor for TracingAccessor {
    #[tracing::instrument]
    fn metadata(&self) -> AccessorMetadata {
        self.inner.metadata()
    }

    #[tracing::instrument]
    async fn create(&self, args: &OpCreate) -> Result<()> {
        self.inner.create(args).await
    }

    #[tracing::instrument]
    async fn read(&self, args: &OpRead) -> Result<BytesReader> {
        self.inner.read(args).await
    }

    #[tracing::instrument(skip(r))]
    async fn write(&self, args: &OpWrite, r: BytesReader) -> Result<u64> {
        self.inner.write(args, r).await
    }

    #[tracing::instrument]
    async fn stat(&self, args: &OpStat) -> Result<ObjectMetadata> {
        self.inner.stat(args).await
    }

    #[tracing::instrument]
    async fn delete(&self, args: &OpDelete) -> Result<()> {
        self.inner.delete(args).await
    }

    #[tracing::instrument]
    async fn list(&self, args: &OpList) -> Result<DirStreamer> {
        self.inner.list(args).await
    }

    #[tracing::instrument]
    fn presign(&self, args: &OpPresign) -> Result<PresignedRequest> {
        self.inner.presign(args)
    }

    #[tracing::instrument]
    async fn create_multipart(&self, args: &OpCreateMultipart) -> Result<String> {
        self.inner.create_multipart(args).await
    }

    #[tracing::instrument(skip(r))]
    async fn write_multipart(&self, args: &OpWriteMultipart, r: BytesReader) -> Result<u64> {
        self.inner.write_multipart(args, r).await
    }

    #[tracing::instrument]
    async fn complete_multipart(&self, args: &OpCompleteMultipart) -> Result<()> {
        self.inner.complete_multipart(args).await
    }

    #[tracing::instrument]
    async fn abort_multipart(&self, args: &OpAbortMultipart) -> Result<()> {
        self.inner.abort_multipart(args).await
    }
}
