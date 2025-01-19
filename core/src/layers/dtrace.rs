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

use std::ffi::CString;
use std::fmt::Debug;
use std::fmt::Formatter;

use bytes::Buf;
use probe::probe_lazy;

use crate::raw::Access;
use crate::raw::*;
use crate::*;

/// Support User Statically-Defined Tracing(aka USDT) on Linux
///
/// This layer is an experimental feature, it will be enabled by `features = ["layers-dtrace"]` in Cargo.toml.
///
/// For now we have following probes:
///
/// ### For Accessor
///
/// 1. ${operation}_start, arguments: path
///     1. create_dir
///     2. read
///     3. write
///     4. stat
///     5. delete
///     6. list
///     7. presign
///     8. blocking_create_dir
///     9. blocking_read
///     10. blocking_write
///     11. blocking_stat
///     12. blocking_delete
///     13. blocking_list
/// 2. ${operation}_end, arguments: path
///     1. create_dir
///     2. read
///     3. write
///     4. stat
///     5. delete
///     6. list
///     7. presign
///     8. blocking_create_dir
///     9. blocking_read
///     10. blocking_write
///     11. blocking_stat
///     12. blocking_delete
///     13. blocking_list
///
/// ### For Reader
///
/// 1. reader_read_start, arguments: path
/// 2. reader_read_ok, arguments: path, length
/// 3. reader_read_error, arguments: path
///
/// ### For BlockingReader
///
/// 1. blocking_reader_read_start, arguments: path
/// 2. blocking_reader_read_ok, arguments: path, length
/// 3. blocking_reader_read_error, arguments: path
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
/// ### For BlockingWriter
///
/// 1. blocking_writer_write_start, arguments: path
/// 2. blocking_writer_write_ok, arguments: path, length
/// 3. blocking_writer_write_error, arguments: path
/// 4. blocking_writer_close_start, arguments: path
/// 5. blocking_writer_close_ok, arguments: path
/// 6. blocking_writer_close_error, arguments: path
///
/// Example:
///
/// ```no_run
/// # use opendal::layers::DtraceLayer;
/// # use opendal::services;
/// # use opendal::Operator;
/// # use opendal::Result;
///
/// # #[tokio::main]
/// # async fn main() -> Result<()> {
/// // `Accessor` provides the low level APIs, we will use `Operator` normally.
/// let op: Operator = Operator::new(services::Fs::default().root("/tmp"))?
///     .layer(DtraceLayer::default())
///     .finish();
///
/// let path = "/tmp/test.txt";
/// for _ in 1..100000 {
///     let bs = vec![0; 64 * 1024 * 1024];
///     op.write(path, bs).await?;
///     op.read(path).await?;
/// }
/// Ok(())
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
#[derive(Default, Debug, Clone)]
pub struct DtraceLayer {}

impl<A: Access> Layer<A> for DtraceLayer {
    type LayeredAccess = DTraceAccessor<A>;
    fn layer(&self, inner: A) -> Self::LayeredAccess {
        DTraceAccessor { inner }
    }
}

#[derive(Clone)]
pub struct DTraceAccessor<A: Access> {
    inner: A,
}

impl<A: Access> Debug for DTraceAccessor<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DTraceAccessor")
            .field("inner", &self.inner)
            .finish_non_exhaustive()
    }
}

impl<A: Access> LayeredAccess for DTraceAccessor<A> {
    type Inner = A;
    type Reader = DtraceLayerWrapper<A::Reader>;
    type BlockingReader = DtraceLayerWrapper<A::BlockingReader>;
    type Writer = DtraceLayerWrapper<A::Writer>;
    type BlockingWriter = DtraceLayerWrapper<A::BlockingWriter>;
    type Lister = A::Lister;
    type BlockingLister = A::BlockingLister;
    type Deleter = A::Deleter;
    type BlockingDeleter = A::BlockingDeleter;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, create_dir_start, c_path.as_ptr());
        let result = self.inner.create_dir(path, args).await;
        probe_lazy!(opendal, create_dir_end, c_path.as_ptr());
        result
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::Reader)> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, read_start, c_path.as_ptr());
        let result = self
            .inner
            .read(path, args)
            .await
            .map(|(rp, r)| (rp, DtraceLayerWrapper::new(r, &path.to_string())));
        probe_lazy!(opendal, read_end, c_path.as_ptr());
        result
    }

    async fn write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::Writer)> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, write_start, c_path.as_ptr());
        let result = self
            .inner
            .write(path, args)
            .await
            .map(|(rp, r)| (rp, DtraceLayerWrapper::new(r, &path.to_string())));

        probe_lazy!(opendal, write_end, c_path.as_ptr());
        result
    }

    async fn stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, stat_start, c_path.as_ptr());
        let result = self.inner.stat(path, args).await;
        probe_lazy!(opendal, stat_end, c_path.as_ptr());
        result
    }

    async fn delete(&self) -> Result<(RpDelete, Self::Deleter)> {
        self.inner.delete().await
    }

    async fn list(&self, path: &str, args: OpList) -> Result<(RpList, Self::Lister)> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, list_start, c_path.as_ptr());
        let result = self.inner.list(path, args).await;
        probe_lazy!(opendal, list_end, c_path.as_ptr());
        result
    }

    async fn presign(&self, path: &str, args: OpPresign) -> Result<RpPresign> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, presign_start, c_path.as_ptr());
        let result = self.inner.presign(path, args).await;
        probe_lazy!(opendal, presign_end, c_path.as_ptr());
        result
    }

    fn blocking_create_dir(&self, path: &str, args: OpCreateDir) -> Result<RpCreateDir> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, blocking_create_dir_start, c_path.as_ptr());
        let result = self.inner.blocking_create_dir(path, args);
        probe_lazy!(opendal, blocking_create_dir_end, c_path.as_ptr());
        result
    }

    fn blocking_read(&self, path: &str, args: OpRead) -> Result<(RpRead, Self::BlockingReader)> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, blocking_read_start, c_path.as_ptr());
        let result = self
            .inner
            .blocking_read(path, args)
            .map(|(rp, r)| (rp, DtraceLayerWrapper::new(r, &path.to_string())));
        probe_lazy!(opendal, blocking_read_end, c_path.as_ptr());
        result
    }

    fn blocking_write(&self, path: &str, args: OpWrite) -> Result<(RpWrite, Self::BlockingWriter)> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, blocking_write_start, c_path.as_ptr());
        let result = self
            .inner
            .blocking_write(path, args)
            .map(|(rp, r)| (rp, DtraceLayerWrapper::new(r, &path.to_string())));
        probe_lazy!(opendal, blocking_write_end, c_path.as_ptr());
        result
    }

    fn blocking_stat(&self, path: &str, args: OpStat) -> Result<RpStat> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, blocking_stat_start, c_path.as_ptr());
        let result = self.inner.blocking_stat(path, args);
        probe_lazy!(opendal, blocking_stat_end, c_path.as_ptr());
        result
    }

    fn blocking_delete(&self) -> Result<(RpDelete, Self::BlockingDeleter)> {
        self.inner.blocking_delete()
    }

    fn blocking_list(&self, path: &str, args: OpList) -> Result<(RpList, Self::BlockingLister)> {
        let c_path = CString::new(path).unwrap();
        probe_lazy!(opendal, blocking_list_start, c_path.as_ptr());
        let result = self.inner.blocking_list(path, args);
        probe_lazy!(opendal, blocking_list_end, c_path.as_ptr());
        result
    }
}

pub struct DtraceLayerWrapper<R> {
    inner: R,
    path: String,
}

impl<R> DtraceLayerWrapper<R> {
    pub fn new(inner: R, path: &String) -> Self {
        Self {
            inner,
            path: path.to_string(),
        }
    }
}

impl<R: oio::Read> oio::Read for DtraceLayerWrapper<R> {
    async fn read(&mut self) -> Result<Buffer> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, reader_read_start, c_path.as_ptr());
        match self.inner.read().await {
            Ok(bs) => {
                probe_lazy!(opendal, reader_read_ok, c_path.as_ptr(), bs.remaining());
                Ok(bs)
            }
            Err(e) => {
                probe_lazy!(opendal, reader_read_error, c_path.as_ptr());
                Err(e)
            }
        }
    }
}

impl<R: oio::BlockingRead> oio::BlockingRead for DtraceLayerWrapper<R> {
    fn read(&mut self) -> Result<Buffer> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, blocking_reader_read_start, c_path.as_ptr());
        self.inner
            .read()
            .map(|bs| {
                probe_lazy!(
                    opendal,
                    blocking_reader_read_ok,
                    c_path.as_ptr(),
                    bs.remaining()
                );
                bs
            })
            .map_err(|e| {
                probe_lazy!(opendal, blocking_reader_read_error, c_path.as_ptr());
                e
            })
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
            .map_err(|err| {
                probe_lazy!(opendal, writer_write_error, c_path.as_ptr());
                err
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
            .map_err(|err| {
                probe_lazy!(opendal, writer_poll_abort_error, c_path.as_ptr());
                err
            })
    }

    async fn close(&mut self) -> Result<Metadata> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, writer_close_start, c_path.as_ptr());
        self.inner
            .close()
            .await
            .map(|_| {
                probe_lazy!(opendal, writer_close_ok, c_path.as_ptr());
            })
            .map_err(|err| {
                probe_lazy!(opendal, writer_close_error, c_path.as_ptr());
                err
            })
    }
}

impl<R: oio::BlockingWrite> oio::BlockingWrite for DtraceLayerWrapper<R> {
    fn write(&mut self, bs: Buffer) -> Result<()> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, blocking_writer_write_start, c_path.as_ptr());
        self.inner
            .write(bs)
            .map(|_| {
                probe_lazy!(opendal, blocking_writer_write_ok, c_path.as_ptr());
            })
            .map_err(|err| {
                probe_lazy!(opendal, blocking_writer_write_error, c_path.as_ptr());
                err
            })
    }

    fn close(&mut self) -> Result<Metadata> {
        let c_path = CString::new(self.path.clone()).unwrap();
        probe_lazy!(opendal, blocking_writer_close_start, c_path.as_ptr());
        self.inner
            .close()
            .map(|_| {
                probe_lazy!(opendal, blocking_writer_close_ok, c_path.as_ptr());
            })
            .map_err(|err| {
                probe_lazy!(opendal, blocking_writer_close_error, c_path.as_ptr());
                err
            })
    }
}
