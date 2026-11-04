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

use std::ffi::c_void;
use std::io::{Read, Seek, SeekFrom};

use ::opendal as core;

use crate::result::opendal_result_reader_seek;

use super::*;

pub const OPENDAL_SEEK_SET: i32 = 0;
pub const OPENDAL_SEEK_CUR: i32 = 1;
pub const OPENDAL_SEEK_END: i32 = 2;

/// \brief A handle that owns an OpenDAL reader.
#[repr(C)]
pub struct opendal_reader {
    /// The owned opendal::blocking::StdReader.
    /// Use this field only to check for NULL.
    inner: *mut c_void,
}

// Overlap one read with the current write in the same task.
async fn copy_buffers(
    mut read: impl AsyncFnMut() -> core::Result<core::Buffer>,
    mut write: impl AsyncFnMut(core::Buffer) -> core::Result<()>,
) -> (u64, core::Result<()>) {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let produce = async move {
        // Reserve capacity before the read to limit prefetch to one buffer.
        while let Ok(permit) = sender.reserve().await {
            let next = read().await;
            let finished = next.as_ref().map_or(true, |buffer| buffer.is_empty());
            permit.send(next);
            if finished {
                break;
            }
        }
    };
    let mut written = 0;
    let result = {
        let consume = async {
            while let Some(next) = receiver.recv().await {
                let buffer = next?;
                if buffer.is_empty() {
                    break;
                }
                let size = buffer.len() as u64;
                write(buffer).await?;
                written += size;
            }
            Ok(())
        };
        tokio::pin!(consume);
        tokio::select! {
            // Handle write errors before polling the next read.
            biased;
            result = &mut consume => result,
            () = produce => consume.await,
        }
    };
    (written, result)
}

impl opendal_reader {
    fn deref_mut(&mut self) -> &mut core::blocking::StdReader {
        // Safety: the inner should never be null once constructed
        // The use-after-free is undefined behavior
        unsafe { &mut *(self.inner as *mut core::blocking::StdReader) }
    }
}

impl opendal_reader {
    pub(crate) fn new(reader: core::blocking::StdReader) -> Self {
        Self {
            inner: Box::into_raw(Box::new(reader)) as _,
        }
    }

    /// \brief Read data from the reader.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_read(
        &mut self,
        buf: *mut u8,
        len: usize,
    ) -> opendal_result_reader_read {
        assert!(!buf.is_null());
        let buf = std::slice::from_raw_parts_mut(buf, len);
        match self.deref_mut().read(buf) {
            Ok(n) => opendal_result_reader_read {
                size: n,
                error: std::ptr::null_mut(),
            },
            Err(e) => opendal_result_reader_read {
                size: 0,
                error: opendal_error::new(
                    core::Error::new(core::ErrorKind::Unexpected, "read failed from reader")
                        .set_source(e),
                ),
            },
        }
    }

    /// \brief Copy native buffers from the reader's current position into the writer.
    ///
    /// Both handles must use the same loaded OpenDAL library.
    /// The handles can belong to different operators.
    ///
    /// The copy reads up to one buffer ahead of the current write.
    /// After a write error, it discards prefetched data and returns without
    /// waiting for a pending read. The reader can consume more bytes than the
    /// copy reports. The byte count includes completed buffer writes, even on
    /// error. It excludes bytes from a failed buffer, including any partial
    /// write. EOF is not an error.
    ///
    /// Both handles stay open. Close the writer to complete the write.
    /// Free both handles after use.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_write_to(
        &mut self,
        writer: &mut opendal_writer,
    ) -> opendal_result_stream_copy {
        let reader = self.deref_mut();
        let runtime = reader.get_handle().clone();
        let reader = match reader.as_async_mut() {
            Ok(reader) => reader,
            Err(err) => {
                return opendal_result_stream_copy {
                    size: 0,
                    error: opendal_error::new(err),
                }
            }
        };
        let writer = match writer.deref_mut().as_async_mut() {
            Ok(writer) => writer,
            Err(err) => {
                return opendal_result_stream_copy {
                    size: 0,
                    error: opendal_error::new(err),
                }
            }
        };
        let (size, result) = runtime.block_on(copy_buffers(
            async || {
                reader.read_buffer(usize::MAX).await.map_err(|err| {
                    core::Error::new(core::ErrorKind::Unexpected, "read failed from reader")
                        .set_source(err)
                })
            },
            async |buffer| writer.write(buffer).await,
        ));
        opendal_result_stream_copy {
            size,
            error: result
                .err()
                .map_or(std::ptr::null_mut(), opendal_error::new),
        }
    }

    /// \brief Seek to an offset, in bytes, in a stream.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_seek(
        &mut self,
        offset: i64,
        whence: i32,
    ) -> opendal_result_reader_seek {
        let pos = match whence {
            _x @ OPENDAL_SEEK_SET => SeekFrom::Start(offset as u64),
            _x @ OPENDAL_SEEK_CUR => SeekFrom::Current(offset),
            _x @ OPENDAL_SEEK_END => SeekFrom::End(offset),
            _ => {
                return opendal_result_reader_seek {
                    pos: 0,
                    error: opendal_error::new(core::Error::new(
                        core::ErrorKind::Unexpected,
                        "undefined whence",
                    )),
                };
            }
        };

        match self.deref_mut().seek(pos) {
            Ok(pos) => opendal_result_reader_seek {
                pos,
                error: std::ptr::null_mut(),
            },
            Err(e) => opendal_result_reader_seek {
                pos: 0,
                error: opendal_error::new(
                    core::Error::new(core::ErrorKind::Unexpected, "seek failed from reader")
                        .set_source(e),
                ),
            },
        }
    }

    /// \brief Frees the heap memory used by the opendal_reader.
    #[no_mangle]
    pub unsafe extern "C" fn opendal_reader_free(ptr: *mut opendal_reader) {
        unsafe {
            if !ptr.is_null() {
                drop(Box::from_raw(
                    (*ptr).inner as *mut core::blocking::StdReader,
                ));
                drop(Box::from_raw(ptr));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn copy_overlaps_read_and_write_on_the_calling_thread() {
        let thread = std::thread::current().id();
        let (read_started, read_ready) = tokio::sync::oneshot::channel();
        let (write_started, write_ready) = tokio::sync::oneshot::channel();
        let mut read_started = Some(read_started);
        let mut read_ready = Some(read_ready);
        let mut write_started = Some(write_started);
        let mut write_ready = Some(write_ready);
        let mut reads = 0;
        let mut writes = 0;
        let copy = copy_buffers(
            async || {
                assert_eq!(std::thread::current().id(), thread);
                reads += 1;
                match reads {
                    1 => Ok(core::Buffer::from("first")),
                    2 => {
                        read_started.take().unwrap().send(()).unwrap();
                        write_ready.take().unwrap().await.unwrap();
                        Ok(core::Buffer::from("next"))
                    }
                    _ => Ok(core::Buffer::new()),
                }
            },
            async |buffer| {
                assert_eq!(std::thread::current().id(), thread);
                writes += 1;
                if writes == 1 {
                    assert_eq!(buffer.to_bytes(), "first");
                    write_started.take().unwrap().send(()).unwrap();
                    read_ready.take().unwrap().await.unwrap();
                } else {
                    assert_eq!(buffer.to_bytes(), "next");
                }
                Ok(())
            },
        );
        let (size, result) = tokio::time::timeout(std::time::Duration::from_secs(1), copy)
            .await
            .expect("read and write must make progress together");
        result.unwrap();
        assert_eq!(size, 9);
        assert_eq!(writes, 2);
    }

    #[tokio::test]
    async fn write_error_does_not_wait_for_stalled_prefetch() {
        struct Dropped<'a>(&'a std::cell::Cell<bool>);
        impl Drop for Dropped<'_> {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let dropped = std::cell::Cell::new(false);
        let (started, ready) = tokio::sync::oneshot::channel();
        let mut started = Some(started);
        let mut ready = Some(ready);
        let mut reads = 0;
        let copy = copy_buffers(
            async || {
                reads += 1;
                if reads == 1 {
                    return Ok(core::Buffer::from("first"));
                }
                let _guard = Dropped(&dropped);
                started.take().unwrap().send(()).unwrap();
                std::future::pending::<core::Result<core::Buffer>>().await
            },
            async |_| {
                ready.take().unwrap().await.unwrap();
                Err(core::Error::new(
                    core::ErrorKind::Unexpected,
                    "write failed",
                ))
            },
        );
        let (size, result) = tokio::time::timeout(std::time::Duration::from_secs(1), copy)
            .await
            .expect("write failure must not wait for prefetch");
        assert_eq!(size, 0);
        assert!(result.is_err());
        assert!(dropped.get());
    }

    #[tokio::test]
    async fn read_error_waits_for_current_write_and_keeps_count() {
        let (started, ready) = tokio::sync::oneshot::channel();
        let mut started = Some(started);
        let mut ready = Some(ready);
        let mut reads = 0;
        let mut completed = false;
        let (size, result) = copy_buffers(
            async || {
                reads += 1;
                if reads == 1 {
                    return Ok(core::Buffer::from("first"));
                }
                started.take().unwrap().send(()).unwrap();
                Err(core::Error::new(core::ErrorKind::Unexpected, "read failed"))
            },
            async |_| {
                ready.take().unwrap().await.unwrap();
                completed = true;
                Ok(())
            },
        )
        .await;
        assert!(completed);
        assert_eq!(size, 5);
        assert_eq!(result.unwrap_err().message(), "read failed");
    }

    #[tokio::test]
    async fn copy_bounds_prefetch_and_counts_completed_buffers() {
        let reads = std::cell::Cell::new(0usize);
        let writes = std::cell::Cell::new(0usize);
        let (size, result) = copy_buffers(
            async || {
                let n = reads.get() + 1;
                reads.set(n);
                assert!(n <= writes.get() + 1);
                Ok(core::Buffer::from("buffer"))
            },
            async |buffer| {
                assert_eq!(buffer.to_bytes(), "buffer");
                let n = writes.get() + 1;
                writes.set(n);
                tokio::task::yield_now().await;
                if n == 4097 {
                    Err(core::Error::new(
                        core::ErrorKind::Unexpected,
                        "write failed",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(size, 4096 * 6);
        assert_eq!(writes.get(), 4097);
    }

    #[test]
    fn stream_copy_preserves_native_buffers() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let source = core::blocking::Operator::new(
            core::Operator::new(core::services::Memory::default()).unwrap(),
        )
        .unwrap();
        let destination = core::blocking::Operator::new(
            core::Operator::new(core::services::Memory::default()).unwrap(),
        )
        .unwrap();

        for use_read_from in [false, true] {
            let first_size = 256 * 1024 + 23;
            let chunks = vec![
                bytes::Bytes::from(vec![1; first_size]),
                bytes::Bytes::from(vec![2; 127]),
            ];
            let addresses = [chunks[0].as_ptr(), chunks[1].as_ptr()];
            source.write("source", core::Buffer::from(chunks)).unwrap();
            let mut reader = Box::new(opendal_reader::new(
                source.reader("source").unwrap().into_std_read(..).unwrap(),
            ));
            let mut writer = Box::new(opendal_writer::new(destination.writer("dest").unwrap()));
            unsafe {
                let seek = reader.opendal_reader_seek(17, OPENDAL_SEEK_SET);
                assert!(seek.error.is_null());
                let mut prefix = [0; 7];
                let read = reader.opendal_reader_read(prefix.as_mut_ptr(), prefix.len());
                assert!(read.error.is_null());
                assert_eq!(read.size, prefix.len());
                assert_eq!(prefix, [1; 7]);
                let result = if use_read_from {
                    writer.opendal_writer_read_from(&mut reader)
                } else {
                    reader.opendal_reader_write_to(&mut writer)
                };
                assert!(result.error.is_null());
                assert_eq!(result.size, (first_size + 127 - 24) as u64);
                let eof = writer.opendal_writer_read_from(&mut reader);
                assert!(eof.error.is_null());
                assert_eq!(eof.size, 0);
                let seek = reader.opendal_reader_seek(0, OPENDAL_SEEK_CUR);
                assert!(seek.error.is_null());
                assert_eq!(seek.pos, (first_size + 127) as u64);
                let seek = reader.opendal_reader_seek(0, OPENDAL_SEEK_SET);
                assert!(seek.error.is_null());
                let read = reader.opendal_reader_read(prefix.as_mut_ptr(), prefix.len());
                assert!(read.error.is_null());
                assert_eq!(read.size, prefix.len());
                assert_eq!(prefix, [1; 7]);
                writer.deref_mut().close().unwrap();
                opendal_reader::opendal_reader_free(Box::into_raw(reader));
                opendal_writer::opendal_writer_free(Box::into_raw(writer));
            }
            source.delete("source").unwrap();

            // The destination owns the buffers after the source is deleted.
            let mut position = 24;
            let copied: Vec<_> = destination.read("dest").unwrap().collect();
            assert_eq!(copied.len(), 2, "copy preserves native chunk boundaries");
            for chunk in copied {
                let (address, remaining, value) = if position < first_size {
                    (
                        addresses[0].wrapping_add(position),
                        first_size - position,
                        1,
                    )
                } else {
                    (
                        addresses[1].wrapping_add(position - first_size),
                        first_size + 127 - position,
                        2,
                    )
                };
                assert_eq!(chunk.as_ptr(), address);
                assert!(chunk.len() <= remaining);
                assert!(chunk.iter().all(|byte| *byte == value));
                position += chunk.len();
            }
            assert_eq!(position, first_size + 127);
        }
    }

    #[test]
    fn stream_copy_reports_empty_input_and_write_errors() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _guard = runtime.enter();
        let op = core::blocking::Operator::new(
            core::Operator::new(core::services::Memory::default()).unwrap(),
        )
        .unwrap();
        op.write("empty", core::Buffer::new()).unwrap();
        op.write("source", "data").unwrap();

        for path in ["empty", "source"] {
            let mut reader = Box::new(opendal_reader::new(
                op.reader(path).unwrap().into_std_read(..).unwrap(),
            ));
            let mut writer = Box::new(opendal_writer::new(op.writer("dest").unwrap()));
            unsafe {
                if path == "source" {
                    writer.deref_mut().close().unwrap();
                }
                let result = reader.opendal_reader_write_to(&mut writer);
                assert_eq!(result.size, 0);
                if path == "empty" {
                    assert!(result.error.is_null());
                    writer.deref_mut().close().unwrap();
                } else {
                    assert!(!result.error.is_null());
                    opendal_error::opendal_error_free(result.error);
                }
                opendal_reader::opendal_reader_free(Box::into_raw(reader));
                opendal_writer::opendal_writer_free(Box::into_raw(writer));
            }
        }
    }
}
