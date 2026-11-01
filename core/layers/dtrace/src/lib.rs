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

#![cfg(target_os = "linux")]
#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]
#![deny(missing_docs)]
use std::ffi::CString;
use std::sync::Arc;

use bytes::Buf;
use opendal_core::raw::*;
use opendal_core::*;
use probe::probe_lazy;

/// `DtraceLayer` supports User Statically-Defined Tracing (USDT) on Linux.
///
/// Enable this experimental layer with `features = ["layers-dtrace"]` in
/// `Cargo.toml`.
///
/// For now we have following probes:
///
/// ### For Service
///
/// 1. ${operation}_start, arguments: path
///     1. create_dir
///     2. read
///     3. write
///     4. stat
///     5. delete
///     6. list
///     7. presign
///
/// 2. ${operation}_end, arguments: path
///     1. create_dir
///     2. read
///     3. write
///     4. stat
///     5. delete
///     6. list
///     7. presign
///
/// ### For Reader
///
/// 1. reader_read_start, arguments: path, range
/// 2. reader_read_ok, arguments: path, range, length
/// 3. reader_read_error, arguments: path, range
///
/// ### For Writer
///
/// 1. writer_write_start, arguments: path
/// 2. writer_write_ok, arguments: path, length
/// 3. writer_write_error, arguments: path
/// 4. writer_abort_start, arguments: path
/// 5. writer_abort_ok, arguments: path
/// 6. writer_abort_error, arguments: path
/// 7. writer_close_start, arguments: path
/// 8. writer_close_ok, arguments: path
/// 9. writer_close_error, arguments: path
///
/// Example:
///
/// ```no_run
/// # use opendal_core::services;
/// # use opendal_core::Operator;
/// # use opendal_core::Result;
/// # use opendal_layer_dtrace::DtraceLayer;
/// #
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// // `Service` provides the low level APIs, we will use `Operator` normally.
/// let op: Operator = Operator::new(services::Memory::default().root("/tmp"))?
///     .layer(DtraceLayer::new());
///
/// let path = "/tmp/test.txt";
/// for _ in 1..100000 {
///     let bs = vec![0; 64 * 1024 * 1024];
///     op.write(path, bs).await?;
///     op.read(path).await?;
/// }
/// # Ok(())
/// # }
/// ```
///
/// Then you can use `readelf -n target/debug/examples/dtrace` to see the probes:
///
/// ```text
/// Displaying notes found in: .note.stapsdt
///   Owner                Data size        Description
///   stapsdt              0x00000039       NT_STAPSDT (SystemTap probe descriptors)
///     Provider: opendal
///     Name: create_dir_start
///     Location: 0x00000000000f8f05, Base: 0x0000000000000000, Semaphore: 0x00000000003649f8
///     Arguments: -8@%rax
///   stapsdt              0x00000037       NT_STAPSDT (SystemTap probe descriptors)
///     Provider: opendal
///     Name: create_dir_end
///     Location: 0x00000000000f9284, Base: 0x0000000000000000, Semaphore: 0x00000000003649fa
///     Arguments: -8@%rax
///   stapsdt              0x0000003c       NT_STAPSDT (SystemTap probe descriptors)
///     Provider: opendal
///     Name: blocking_list_start
///     Location: 0x00000000000f9487, Base: 0x0000000000000000, Semaphore: 0x0000000000364a28
///     Arguments: -8@%rax
///   stapsdt              0x0000003a       NT_STAPSDT (SystemTap probe descriptors)
///     Provider: opendal
///     Name: blocking_list_end
///     Location: 0x00000000000f9546, Base: 0x0000000000000000, Semaphore: 0x0000000000364a2a
///     Arguments: -8@%rax
///   stapsdt              0x0000003c       NT_STAPSDT (SystemTap probe descriptors)
/// ```
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct DtraceLayer {}

impl DtraceLayer {
    /// Create a new [`DtraceLayer`].
    pub fn new() -> Self {
        Self::default()
    }
}

impl Layer for DtraceLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(self.layer(inner))
    }
}

impl DtraceLayer {
    fn layer(&self, inner: Servicer) -> DTraceService {
        DTraceService { inner }
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct DTraceService {
    inner: Servicer,
}

impl Service for DTraceService {
    type Reader = DtraceLayerWrapper<oio::Reader>;
    type Writer = DtraceLayerWrapper<oio::Writer>;
    type Lister = oio::Lister;
    type Deleter = oio::Deleter;
    type Copier = oio::Copier;
    type Composer = oio::Composer;

    fn info(&self) -> ServiceInfo {
        self.inner.info()
    }

    fn capability(&self) -> Capability {
        self.inner.capability()
    }

    fn compose(&self, ctx: &OperationContext, to: &str, args: OpCompose) -> Result<Self::Composer> {
        self.inner.compose(ctx, to, args)
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, create_dir_start, c_path.as_ptr());
        let result = self.inner.create_dir(ctx, path, args).await;
        probe_lazy!(opendal, create_dir_end, c_path.as_ptr());
        result
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, read_start, c_path.as_ptr());
        let result = self
            .inner
            .read(ctx, path, args)
            .map(|r| DtraceLayerWrapper::new(r, path));
        probe_lazy!(opendal, read_end, c_path.as_ptr());
        result
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, write_start, c_path.as_ptr());
        let result = self
            .inner
            .write(ctx, path, args)
            .map(|r| DtraceLayerWrapper::new(r, path));

        probe_lazy!(opendal, write_end, c_path.as_ptr());
        result
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
    ) -> Result<Self::Copier> {
        let c_from = CString::new(from).unwrap();
        probe_lazy!(opendal, copy_start, c_from.as_ptr());
        let result = self.inner.copy(ctx, from, to, args);
        probe_lazy!(opendal, copy_end, c_from.as_ptr());
        result
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.inner.rename(ctx, from, to, args).await
    }

    async fn restore(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpRestore,
    ) -> Result<RpRestore> {
        self.inner.restore(ctx, path, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, stat_start, c_path.as_ptr());
        let result = self.inner.stat(ctx, path, args).await;
        probe_lazy!(opendal, stat_end, c_path.as_ptr());
        result
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        self.inner.delete(ctx)
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, list_start, c_path.as_ptr());
        let result = self.inner.list(ctx, path, args);
        probe_lazy!(opendal, list_end, c_path.as_ptr());
        result
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, presign_start, c_path.as_ptr());
        let result = self.inner.presign(ctx, path, args).await;
        probe_lazy!(opendal, presign_end, c_path.as_ptr());
        result
    }
}

#[doc(hidden)]
pub struct DtraceLayerWrapper<R> {
    inner: R,
    path: String,
    range: Option<BytesRange>,
}

impl<R> DtraceLayerWrapper<R> {
    fn new(inner: R, path: &str) -> Self {
        Self::with_range(inner, path, None)
    }

    fn with_range(inner: R, path: &str, range: Option<BytesRange>) -> Self {
        Self {
            inner,
            path: path.to_string(),
            range,
        }
    }

    fn range_label(&self) -> String {
        self.range
            .map(|range| range.to_string())
            .unwrap_or_default()
    }
}

impl<R: oio::ReadStream> oio::ReadStream for DtraceLayerWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        let c_path = CString::new(self.path.clone()).unwrap();
        let c_range = CString::new(self.range_label()).unwrap();
        probe_lazy!(
            opendal,
            reader_read_start,
            c_path.as_ptr(),
            c_range.as_ptr()
        );
        match self.inner.read().await {
            Ok(bs) => {
                probe_lazy!(
                    opendal,
                    reader_read_ok,
                    c_path.as_ptr(),
                    c_range.as_ptr(),
                    bs.remaining()
                );
                Ok(bs)
            }
            Err(e) => {
                probe_lazy!(
                    opendal,
                    reader_read_error,
                    c_path.as_ptr(),
                    c_range.as_ptr()
                );
                Err(e)
            }
        }
    }
}

impl<R: oio::Read> oio::Read for DtraceLayerWrapper<R> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        let c_path = CString::new(self.path.clone()).unwrap();
        let c_range = CString::new(range.to_string()).unwrap();
        probe_lazy!(
            opendal,
            reader_read_start,
            c_path.as_ptr(),
            c_range.as_ptr()
        );
        match self.inner.open(range).await {
            Ok((rp, stream)) => {
                probe_lazy!(
                    opendal,
                    reader_read_ok,
                    c_path.as_ptr(),
                    c_range.as_ptr(),
                    0
                );
                Ok((
                    rp,
                    Box::new(DtraceLayerWrapper::with_range(
                        stream,
                        &self.path,
                        Some(range),
                    )) as Box<dyn oio::ReadStreamDyn>,
                ))
            }
            Err(e) => {
                probe_lazy!(
                    opendal,
                    reader_read_error,
                    c_path.as_ptr(),
                    c_range.as_ptr()
                );
                Err(e)
            }
        }
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        let c_path = CString::new(self.path.clone()).unwrap();
        let c_range = CString::new(range.to_string()).unwrap();
        probe_lazy!(
            opendal,
            reader_read_start,
            c_path.as_ptr(),
            c_range.as_ptr()
        );
        match self.inner.read(range).await {
            Ok((rp, buffer)) => {
                probe_lazy!(
                    opendal,
                    reader_read_ok,
                    c_path.as_ptr(),
                    c_range.as_ptr(),
                    buffer.len()
                );
                Ok((rp, buffer))
            }
            Err(e) => {
                probe_lazy!(
                    opendal,
                    reader_read_error,
                    c_path.as_ptr(),
                    c_range.as_ptr()
                );
                Err(e)
            }
        }
    }
}

impl<R: oio::Write> oio::Write for DtraceLayerWrapper<R> {
    async fn write(&mut self, bs: Buffer) -> Result<()> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, writer_write_start, c_path.as_ptr());
        self.inner
            .write(bs)
            .await
            .map(|_| {
                probe_lazy!(opendal, writer_write_ok, c_path.as_ptr());
            })
            .inspect_err(|_| {
                probe_lazy!(opendal, writer_write_error, c_path.as_ptr());
            })
    }

    async fn copy_from(&mut self, path: &str, args: OpRead, range: BytesRange) -> Result<()> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, writer_write_start, c_path.as_ptr());
        self.inner
            .copy_from(path, args, range)
            .await
            .map(|_| {
                probe_lazy!(opendal, writer_write_ok, c_path.as_ptr());
            })
            .inspect_err(|_| {
                probe_lazy!(opendal, writer_write_error, c_path.as_ptr());
            })
    }

    async fn abort(&mut self) -> Result<()> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, writer_poll_abort_start, c_path.as_ptr());
        self.inner
            .abort()
            .await
            .map(|_| {
                probe_lazy!(opendal, writer_poll_abort_ok, c_path.as_ptr());
            })
            .inspect_err(|_| {
                probe_lazy!(opendal, writer_poll_abort_error, c_path.as_ptr());
            })
    }

    async fn close(&mut self) -> Result<Metadata> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, writer_close_start, c_path.as_ptr());
        self.inner
            .close()
            .await
            .inspect(|_| {
                probe_lazy!(opendal, writer_close_ok, c_path.as_ptr());
            })
            .inspect_err(|_| {
                probe_lazy!(opendal, writer_close_error, c_path.as_ptr());
            })
    }
}
