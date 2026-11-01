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

use super::core::{BucketOperation, HfCore, LfsFile};
use opendal_core::raw::*;
use opendal_core::*;
use xet::error::XetError;
use xet::xet_session::{Sha256Policy, XetFileInfo, XetStreamUpload, XetUploadCommit};

/// Writer that always uses the XET protocol for uploads.
pub struct HfWriter {
    core: Arc<HfCore>,
    ctx: OperationContext,
    path: String,
    xet_commit: XetUploadCommit,
    xet_stream: XetStreamUpload,
    // RetryLayer can replay Writer::close after a temporary commit failure,
    // so writer finalization must be re-entrant and reuse the first finish result.
    finished_info: Option<XetFileInfo>,
}

/// Lazily creates [`HfWriter`] so `Service::write` can stay synchronous.
pub struct HfLazyWriter {
    core: Arc<HfCore>,
    ctx: OperationContext,
    path: String,
    inner: Option<HfWriter>,
}

impl HfLazyWriter {
    pub fn new(core: Arc<HfCore>, ctx: OperationContext, path: String) -> Self {
        Self {
            core,
            ctx,
            path,
            inner: None,
        }
    }

    async fn inner(&mut self) -> Result<&mut HfWriter> {
        if self.inner.is_none() {
            let writer =
                HfWriter::try_new(self.core.clone(), self.ctx.clone(), self.path.clone()).await?;
            self.inner = Some(writer);
        }

        Ok(self.inner.as_mut().expect("hf writer must be initialized"))
    }
}

impl oio::Write for HfLazyWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.inner().await?.write(bs).await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.inner().await?.close().await
    }

    async fn abort(&mut self) -> Result<()> {
        match &mut self.inner {
            Some(w) => w.abort().await,
            None => Ok(()),
        }
    }
}

impl HfWriter {
    /// Create a new writer for the given path.
    ///
    /// All uploads go through the XET protocol regardless of file size.
    /// An XET upload commit and stream are created eagerly so that
    /// subsequent [`write`](oio::Write::write) calls can stream data
    /// directly into the XET CAS.
    pub async fn try_new(core: Arc<HfCore>, ctx: OperationContext, path: String) -> Result<Self> {
        let commit = core.xet_upload_commit(&ctx).await?;
        let stream = commit
            .upload_stream(None, Sha256Policy::Compute)
            .await
            .map_err(|err| {
                Error::new(ErrorKind::Unexpected, "failed to start xet upload stream")
                    .set_source(err)
            })?;
        Ok(HfWriter {
            core,
            ctx,
            path,
            xet_commit: commit,
            xet_stream: stream,
            finished_info: None,
        })
    }

    /// Finalize the XET upload and register the file with HF.
    ///
    /// 1. Commit data to XET CAS (may already be done by `stream.finish()`)
    /// 2. Register the file via git LFS commit or bucket batch API
    ///
    /// Retries on transient CAS propagation delays.
    async fn commit(&mut self, file_info: &XetFileInfo) -> Result<Metadata> {
        // Finalize the XET CAS upload. May return AlreadyCompleted
        // if stream.finish() already committed internally.
        match self.xet_commit.commit().await {
            Ok(_) | Err(XetError::AlreadyCompleted) => {}
            Err(e) => {
                return Err(
                    Error::new(ErrorKind::Unexpected, "failed to commit xet upload").set_source(e),
                );
            }
        }

        let content_length = file_info
            .file_size()
            .expect("file_size must be set after finish()");
        let meta = {
            let mut metadata = MetadataBuilder::unknown();
            metadata.set_file(content_length);
            metadata.build()
        };

        let repo_path = self.core.repo_path(&self.path);
        if self.core.repo.is_bucket() {
            let xet_hash = file_info.hash().to_string();
            self.core
                .commit_bucket(
                    &self.ctx,
                    vec![BucketOperation::AddFile {
                        path: repo_path,
                        xet_hash,
                    }],
                )
                .await?;
        } else {
            let sha256 = file_info.sha256().ok_or_else(|| {
                Error::new(ErrorKind::Unexpected, "xet upload returned no sha256")
            })?;
            let lfs_file = LfsFile {
                path: repo_path,
                oid: sha256.to_string(),
                algo: "sha256".to_string(),
                size: content_length,
            };
            self.core
                .commit_git(&self.ctx, vec![], vec![lfs_file], vec![], vec![])
                .await?;
        }

        Ok(meta)
    }
}

impl oio::Write for HfWriter {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        self.xet_stream.write(bs.to_bytes()).await.map_err(|err| {
            Error::new(ErrorKind::Unexpected, "failed to write xet chunk").set_source(err)
        })
    }

    async fn close(&mut self) -> Result<Metadata> {
        let file_info = match self.finished_info.clone() {
            Some(file_info) => file_info,
            None => match self.xet_stream.finish().await {
                Ok(file_meta) => {
                    // Cache the finalized metadata so a retried close can skip
                    // finish() and only replay the registration step.
                    let file_info = file_meta.xet_info;
                    self.finished_info = Some(file_info.clone());
                    file_info
                }
                Err(XetError::AlreadyCompleted) => {
                    return Err(Error::new(
                        ErrorKind::Unexpected,
                        "xet upload finished before metadata was cached",
                    ));
                }
                Err(err) => {
                    return Err(
                        Error::new(ErrorKind::Unexpected, "failed to finish xet upload")
                            .set_source(err),
                    );
                }
            },
        };

        self.commit(&file_info).await
    }

    async fn abort(&mut self) -> Result<()> {
        self.xet_stream.abort();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::backend::test_utils::testing_bucket_operator;

    /// Bucket writes always use XET and commit via NDJSON batch — this code
    /// path is not covered by behavior tests unless run with a bucket config.
    /// Requires HF_OPENDAL_BUCKET and HF_OPENDAL_TOKEN.
    #[tokio::test]
    #[ignore]
    async fn test_bucket_write_roundtrip() {
        let op = testing_bucket_operator();
        let path = "tests/bucket-roundtrip.bin";
        let content = b"Binary content for bucket roundtrip test";

        op.write(path, content.as_slice())
            .await
            .expect("bucket write should succeed");

        let data = op.read(path).await.expect("read should succeed");
        assert_eq!(data.to_bytes().as_ref(), content);

        let _ = op.delete(path).await;
    }
}
