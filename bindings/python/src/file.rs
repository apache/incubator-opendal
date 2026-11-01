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

use std::io::BufRead;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::ops::DerefMut;
use std::sync::Arc;

use asyncband::mutex::Mutex;
use futures::AsyncSeekExt;
use futures::SinkExt;
use pyo3::IntoPyObjectExt;
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::PyIOError;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyBytes;
use pyo3_async_runtimes::tokio::future_into_py;
use pyo3_async_runtimes::tokio::get_runtime;

use crate::*;

/// A file-like object for reading and writing data.
///
/// Created by the `open` method of the `Operator` class.
///
/// Storage I/O releases the Python GIL while waiting. Concurrent operations on
/// the same file are rejected; use separate files for I/O from multiple threads.
#[pyclass(module = "opendal.file")]
pub struct File(FileState);

enum FileState {
    Reader(Box<ocore::blocking::StdReader>),
    Writer(ocore::BufferSink),
    Closed,
}

impl File {
    pub fn new_reader(reader: ocore::blocking::StdReader) -> Self {
        Self(FileState::Reader(Box::new(reader)))
    }

    pub fn new_writer(writer: ocore::Writer) -> Self {
        Self(FileState::Writer(writer.into_sink()))
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if matches!(self.0, FileState::Writer(_)) {
            // Pending operations can own runtime resources even without close.
            let _guard = get_runtime().enter();
            drop(std::mem::replace(&mut self.0, FileState::Closed));
        }
    }
}
#[pymethods]
impl File {
    /// Read at most `size` bytes from this file.
    ///
    /// If `size` is not specified, read until EOF.
    ///
    /// Parameters
    /// ----------
    /// size : int, optional
    ///     The maximum number of bytes to read.
    ///
    /// Notes
    /// -----
    /// Fewer bytes may be returned than requested, read in a loop
    /// to ensure all bytes are read.
    ///
    /// Returns
    /// -------
    /// bytes
    ///     The bytes read from this file.
    #[pyo3(signature = (size=None) -> "bytes")]
    pub fn read<'p>(
        &'p mut self,
        py: Python<'p>,
        size: Option<usize>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let reader = match &mut self.0 {
            FileState::Reader(r) => r,
            FileState::Writer(_) => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on write only file.",
                ));
            }
            FileState::Closed => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on closed file.",
                ));
            }
        };

        let buffer = py
            .detach(|| match size {
                Some(size) => reader.read_buffer(size),
                None => reader.read_to_end_buffer(),
            })
            .map_err(|err| PyIOError::new_err(err.to_string()))?;

        buffer_into_py_bytes(py, buffer).map(Bound::into_any)
    }

    /// Read one line from this file.
    ///
    /// If `size` is not specified, read until newline.
    ///
    /// Parameters
    /// ----------
    /// size : int, optional
    ///     The maximum number of bytes to read.
    ///
    /// Notes
    /// -----
    /// Retains newline characters after each line, unless
    /// the file’s last line has no terminating newline.
    ///
    /// Returns
    /// -------
    /// bytes
    ///     The bytes read from this file.
    #[pyo3(signature = (size=None) -> "bytes")]
    pub fn readline<'p>(
        &'p mut self,
        py: Python<'p>,
        size: Option<usize>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let reader = match &mut self.0 {
            FileState::Reader(r) => r,
            FileState::Writer(_) => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on write only file.",
                ));
            }
            FileState::Closed => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on closed file.",
                ));
            }
        };

        let buffer = py
            .detach(|| -> std::io::Result<Vec<u8>> {
                let mut buffer = Vec::new();
                match size {
                    None => reader.read_until(b'\n', &mut buffer)?,
                    Some(size) => reader.take(size as u64).read_until(b'\n', &mut buffer)?,
                };
                Ok(buffer)
            })
            .map_err(|err| PyIOError::new_err(err.to_string()))?;

        buffer_into_py_bytes(py, buffer.into()).map(Bound::into_any)
    }

    /// Read bytes into a pre-allocated buffer.
    ///
    /// Parameters
    /// ----------
    /// buffer : bytes | bytearray
    ///     A writable, pre-allocated buffer to read into.
    ///
    /// Returns
    /// -------
    /// int
    ///     The number of bytes read.
    #[pyo3(signature = (buffer: "bytearray | memoryview"))]
    pub fn readinto(&mut self, py: Python<'_>, buffer: PyBuffer<u8>) -> PyResult<usize> {
        let reader = match &mut self.0 {
            FileState::Reader(r) => r,
            FileState::Writer(_) => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on write only file.",
                ));
            }
            FileState::Closed => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on closed file.",
                ));
            }
        };

        if buffer.readonly() {
            return Err(PyIOError::new_err("Buffer is not writable."));
        }

        if !buffer.is_c_contiguous() {
            return Err(PyIOError::new_err("Buffer is not C contiguous."));
        }

        let size = buffer.len_bytes();
        // Wait using owned storage, without exposing the Python buffer to I/O.
        let data = py.detach(|| reader.read_buffer(size))?;
        let len = data.len();
        let target = buffer.as_mut_slice(py).expect("buffer was validated above");
        let mut offset = 0;
        for chunk in data {
            for (dst, src) in target[offset..offset + chunk.len()]
                .iter()
                .zip(chunk.iter())
            {
                dst.set(*src);
            }
            offset += chunk.len();
        }
        Ok(len)
    }

    /// Write bytes to this file.
    ///
    /// Parameters
    /// ----------
    /// bs : bytes
    ///     The bytes to write to the file.
    ///
    /// Returns
    /// -------
    /// int
    ///     The number of bytes written.
    #[pyo3(signature = (bs: "bytes"))]
    pub fn write(&mut self, py: Python<'_>, bs: &Bound<PyBytes>) -> PyResult<usize> {
        let writer = match &mut self.0 {
            FileState::Reader(_) => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on read only file.",
                ));
            }
            FileState::Writer(w) => w,
            FileState::Closed => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on closed file.",
                ));
            }
        };

        let bs = py_bytes_like_into_buffer(bs.as_any())?;
        let len = bs.len();
        if len == 0 {
            return Ok(0);
        }
        py.detach(|| get_runtime().handle().block_on(writer.send(bs)))
            .map(|_| len)
            .map_err(|err| PyIOError::new_err(err.to_string()))
    }

    /// Change the position of this file to the given byte offset.
    ///
    /// Parameters
    /// ----------
    /// pos : int
    ///     The byte offset (position) to set.
    /// whence : int, optional
    ///     The reference point for the offset.
    ///     0: start of file (default); 1: current position; 2: end of file.
    ///
    /// Notes
    /// -----
    /// Unbounded readable files allow non-negative positions beyond EOF. Reads
    /// from those positions return empty bytes when the backing service reports
    /// an unsatisfied unbounded range. Other service errors propagate. Explicit
    /// bounded views reject positions beyond their range.
    ///
    /// Returns
    /// -------
    /// int
    ///     The new absolute position.
    #[pyo3(signature = (pos, whence = 0))]
    pub fn seek(&mut self, py: Python<'_>, pos: i64, whence: u8) -> PyResult<u64> {
        if !self.seekable()? {
            return Err(PyIOError::new_err(
                "Seek operation is not supported by the backing service.",
            ));
        }
        let reader = match &mut self.0 {
            FileState::Reader(r) => r,
            FileState::Writer(_) => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on write only file.",
                ));
            }
            FileState::Closed => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on closed file.",
                ));
            }
        };

        let whence = match whence {
            0 => SeekFrom::Start(pos as u64),
            1 => SeekFrom::Current(pos),
            2 => SeekFrom::End(pos),
            _ => return Err(PyValueError::new_err("invalid whence")),
        };

        py.detach(|| reader.seek(whence))
            .map_err(|err| PyIOError::new_err(err.to_string()))
    }

    /// Return the current position of this file.
    ///
    /// Returns
    /// -------
    /// int
    ///     The current absolute position.
    pub fn tell(&mut self, py: Python<'_>) -> PyResult<u64> {
        let reader = match &mut self.0 {
            FileState::Reader(r) => r,
            FileState::Writer(_) => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on write only file.",
                ));
            }
            FileState::Closed => {
                return Err(PyIOError::new_err(
                    "I/O operation failed for reading on closed file.",
                ));
            }
        };

        py.detach(|| reader.stream_position())
            .map_err(|err| PyIOError::new_err(err.to_string()))
    }

    /// Close this file.
    ///
    /// This also flushes write buffers, if applicable.
    ///
    /// Notes
    /// -----
    /// A closed file cannot be used for further I/O operations.
    fn close(&mut self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let _guard = get_runtime().enter();
            if let FileState::Writer(w) = &mut self.0 {
                get_runtime().handle().block_on(w.close())?;
            }
            self.0 = FileState::Closed;
            Ok(())
        })
        .map_err(format_pyerr)
    }

    pub fn __enter__(slf: PyRef<'_, Self>) -> Py<Self> {
        slf.into()
    }

    #[allow(unused_variables)]
    #[pyo3(signature = (
        exc_type: "type[BaseException] | None",
        exc_value: "BaseException | None",
        traceback: "types.TracebackType | None"))]
    pub fn __exit__(
        &mut self,
        py: Python<'_>,
        exc_type: Py<PyAny>,
        exc_value: Py<PyAny>,
        traceback: Py<PyAny>,
    ) -> PyResult<()> {
        self.close(py)
    }

    /// Flush the underlying writer.
    ///
    /// Notes
    /// -----
    /// Passes pending input to the core writer. Storage chunks can remain
    /// buffered until more data arrives or `close()` completes the write.
    /// Is a no-op if the file is not `writable`.
    pub fn flush(&mut self, py: Python<'_>) -> PyResult<()> {
        if matches!(self.0, FileState::Reader(_)) {
            Ok(())
        } else if let FileState::Writer(w) = &mut self.0 {
            match py.detach(|| get_runtime().handle().block_on(w.flush())) {
                Ok(_) => Ok(()),
                Err(e) => Err(std::io::Error::from(e).into()),
            }
        } else {
            Ok(())
        }
    }

    /// Whether this file can be read from.
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if this file can be read from.
    pub fn readable(&self) -> PyResult<bool> {
        Ok(matches!(self.0, FileState::Reader(_)))
    }

    /// Whether this file can be written to.
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if this file can be written to.
    pub fn writable(&self) -> PyResult<bool> {
        Ok(matches!(self.0, FileState::Writer(_)))
    }

    /// Whether this file can be repositioned.
    ///
    /// Notes
    /// -----
    /// This is only applicable to *readable* files.
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if this file can be repositioned.
    pub fn seekable(&self) -> PyResult<bool> {
        match &self.0 {
            FileState::Reader(_) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Whether this file is closed.
    ///
    /// Returns
    /// -------
    /// bool
    ///     True if this file is closed.
    #[getter]
    pub fn closed(&self) -> PyResult<bool> {
        Ok(matches!(self.0, FileState::Closed))
    }
}

/// An async file-like object for reading and writing data.
///
/// Created by the `open` method of the `AsyncOperator` class.
#[pyclass(module = "opendal.file")]
pub struct AsyncFile(Arc<Mutex<AsyncFileState>>);

enum AsyncFileState {
    Reader(Box<ocore::FuturesAsyncReader>),
    Writer(ocore::BufferSink),
    Closed,
}

impl AsyncFile {
    pub fn new_reader(reader: ocore::FuturesAsyncReader) -> Self {
        Self(Arc::new(Mutex::new(AsyncFileState::Reader(Box::new(
            reader,
        )))))
    }

    pub fn new_writer(writer: ocore::Writer) -> Self {
        Self(Arc::new(Mutex::new(AsyncFileState::Writer(
            writer.into_sink(),
        ))))
    }
}
#[pymethods]
impl AsyncFile {
    /// Read at most `size` bytes from this file asynchronously.
    ///
    /// If `size` is not specified, read until EOF.
    ///
    /// Parameters
    /// ----------
    /// size : int, optional
    ///     The maximum number of bytes to read.
    ///
    /// Notes
    /// -----
    /// Fewer bytes may be returned than requested, read in a loop
    /// to ensure all bytes are read.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns the bytes read from the stream.
    #[pyo3(signature = (size=None) -> "collections.abc.Awaitable[bytes]")]
    pub fn read<'p>(&'p self, py: Python<'p>, size: Option<usize>) -> PyResult<Bound<'p, PyAny>> {
        let state = self.0.clone();

        future_into_py(py, async move {
            let mut guard = state.lock().await;
            let reader = match guard.deref_mut() {
                AsyncFileState::Reader(r) => r,
                AsyncFileState::Writer(_) => {
                    return Err(PyIOError::new_err(
                        "I/O operation failed for reading on write only file.",
                    ));
                }
                _ => {
                    return Err(PyIOError::new_err(
                        "I/O operation failed for reading on closed file.",
                    ));
                }
            };

            let buffer = match size {
                Some(size) => reader.read_buffer(size).await,
                None => reader.read_to_end_buffer().await,
            }
            .map_err(|err| PyIOError::new_err(err.to_string()))?;

            Python::attach(|py| buffer_into_py_bytes(py, buffer).map(Bound::unbind))
        })
    }

    /// Write bytes to this file asynchronously.
    ///
    /// Parameters
    /// ----------
    /// bs : bytes
    ///     The bytes to write to the file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns the number of bytes written.
    #[pyo3(signature = (bs: "bytes") -> "collections.abc.Awaitable[int]")]
    pub fn write<'p>(
        &'p mut self,
        py: Python<'p>,
        bs: &Bound<PyBytes>,
    ) -> PyResult<Bound<'p, PyAny>> {
        let state = self.0.clone();
        let bs = py_bytes_like_into_buffer(bs.as_any())?;

        future_into_py(py, async move {
            let mut guard = state.lock().await;
            let writer = match guard.deref_mut() {
                AsyncFileState::Reader(_) => {
                    return Err(PyIOError::new_err(
                        "I/O operation failed for reading on read only file.",
                    ));
                }
                AsyncFileState::Writer(w) => w,
                _ => {
                    return Err(PyIOError::new_err(
                        "I/O operation failed for reading on closed file.",
                    ));
                }
            };

            let len = bs.len();
            if len == 0 {
                return Ok(0);
            }
            writer
                .send(bs)
                .await
                .map(|_| len)
                .map_err(|err| PyIOError::new_err(err.to_string()))
        })
    }

    /// Change the position of this file to the given byte offset.
    ///
    /// Parameters
    /// ----------
    /// pos : int
    ///     The byte offset (position) to set.
    /// whence : int, optional
    ///     The reference point for the offset.
    ///     0: start of file (default); 1: current position; 2: end of file.
    ///
    /// Notes
    /// -----
    /// Unbounded readable files allow non-negative positions beyond EOF. Reads
    /// from those positions return empty bytes when the backing service reports
    /// an unsatisfied unbounded range. Other service errors propagate. Explicit
    /// bounded views reject positions beyond their range.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns the current absolute position.
    #[pyo3(signature = (pos, whence = 0) -> "collections.abc.Awaitable[int]")]
    pub fn seek<'p>(
        &'p mut self,
        py: Python<'p>,
        pos: i64,
        whence: u8,
    ) -> PyResult<Bound<'p, PyAny>> {
        let state = self.0.clone();

        let whence = match whence {
            0 => SeekFrom::Start(pos as u64),
            1 => SeekFrom::Current(pos),
            2 => SeekFrom::End(pos),
            _ => return Err(PyValueError::new_err("invalid whence")),
        };

        future_into_py(py, async move {
            let mut guard = state.lock().await;
            let reader = match guard.deref_mut() {
                AsyncFileState::Reader(r) => r,
                AsyncFileState::Writer(_) => {
                    return Err(PyIOError::new_err(
                        "I/O operation failed for reading on write only file.",
                    ));
                }
                _ => {
                    return Err(PyIOError::new_err(
                        "I/O operation failed for reading on closed file.",
                    ));
                }
            };

            let pos = reader
                .seek(whence)
                .await
                .map_err(|err| PyIOError::new_err(err.to_string()))?;
            Ok(pos)
        })
        .and_then(|pos| pos.into_bound_py_any(py))
    }

    /// Return the current position of this file.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns the current absolute position.
    #[pyo3(signature = () -> "collections.abc.Awaitable[int]")]
    pub fn tell<'p>(&'p mut self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let state = self.0.clone();

        future_into_py(py, async move {
            let mut guard = state.lock().await;
            let reader = match guard.deref_mut() {
                AsyncFileState::Reader(r) => r,
                AsyncFileState::Writer(_) => {
                    return Err(PyIOError::new_err(
                        "I/O operation failed for reading on write only file.",
                    ));
                }
                _ => {
                    return Err(PyIOError::new_err(
                        "I/O operation failed for reading on closed file.",
                    ));
                }
            };

            let pos = reader
                .stream_position()
                .await
                .map_err(|err| PyIOError::new_err(err.to_string()))?;
            Ok(pos)
        })
        .and_then(|pos| pos.into_bound_py_any(py))
    }

    /// Close this file.
    ///
    /// This also flushes write buffers, if applicable.
    ///
    /// Notes
    /// -----
    /// A closed file cannot be used for further I/O operations.
    #[pyo3(signature = () -> "collections.abc.Awaitable[None]")]
    fn close<'p>(&'p mut self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let state = self.0.clone();
        future_into_py(py, async move {
            let mut state = state.lock().await;
            if let AsyncFileState::Writer(w) = &mut *state {
                w.close().await.map_err(format_pyerr)?;
            }
            *state = AsyncFileState::Closed;
            Ok(())
        })
    }

    // `typing_extensions.Self` because `typing.Self` is 3.11+ and the floor is 3.10.
    #[pyo3(signature = () -> "collections.abc.Awaitable[typing_extensions.Self]")]
    fn __aenter__<'a>(slf: PyRef<'a, Self>, py: Python<'a>) -> PyResult<Bound<'a, PyAny>> {
        let slf = slf.into_py_any(py)?;
        future_into_py(py, async move { Ok(slf) })
    }

    #[allow(unused_variables)]
    #[pyo3(signature = (
        exc_type: "type[BaseException] | None",
        exc_value: "BaseException | None",
        traceback: "types.TracebackType | None") -> "None")]
    fn __aexit__<'a>(
        &'a mut self,
        py: Python<'a>,
        exc_type: &Bound<'a, PyAny>,
        exc_value: &Bound<'a, PyAny>,
        traceback: &Bound<'a, PyAny>,
    ) -> PyResult<Bound<'a, PyAny>> {
        self.close(py)
    }

    /// Whether this file can be read from.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns True if this file can be read from.
    #[pyo3(signature = () -> "collections.abc.Awaitable[bool]")]
    pub fn readable<'p>(&'p self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let state = self.0.clone();
        future_into_py(py, async move {
            let state = state.lock().await;
            Ok(matches!(*state, AsyncFileState::Reader(_)))
        })
    }

    /// Whether this file can be written to.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns True if this file can be written to.
    #[pyo3(signature = () -> "collections.abc.Awaitable[bool]")]
    pub fn writable<'p>(&'p self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let state = self.0.clone();
        future_into_py(py, async move {
            let state = state.lock().await;
            Ok(matches!(*state, AsyncFileState::Writer(_)))
        })
    }

    /// Whether this file can be repositioned.
    ///
    /// Notes
    /// -----
    /// This is only applicable to *readable* files.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns True if this file can be repositioned.
    #[pyo3(signature = () -> "collections.abc.Awaitable[bool]")]
    pub fn seekable<'p>(&'p self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        if true {
            self.readable(py)
        } else {
            future_into_py(py, async move { Ok(false) })
        }
    }

    /// Whether this file is closed.
    ///
    /// Returns
    /// -------
    /// coroutine
    ///     An awaitable that returns True if this file is closed.
    #[getter]
    pub fn closed<'p>(&'p self, py: Python<'p>) -> PyResult<Bound<'p, PyAny>> {
        let state = self.0.clone();
        future_into_py(py, async move {
            let state = state.lock().await;
            Ok(matches!(*state, AsyncFileState::Closed))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use asyncband::semaphore::Semaphore;
    use futures::poll;
    use ocore::layers::ConcurrentLimitLayer;

    use super::*;

    #[tokio::test]
    async fn test_cancelled_send_keeps_owned_buffer() {
        Python::initialize();
        let (buffer, source_ptr) = Python::attach(|py| {
            let source = PyBytes::new(py, b"owned Python bytes");
            let source_ptr = source.as_bytes().as_ptr();
            let buffer = py_bytes_like_into_buffer(source.as_any()).unwrap();
            (buffer, source_ptr)
        });
        let semaphore = Arc::new(Semaphore::new(0));
        let op = ocore::Operator::via_iter("memory", [])
            .unwrap()
            .layer(ConcurrentLimitLayer::with_semaphore(semaphore.clone()));
        let file = AsyncFile::new_writer(op.writer_with("cancelled").chunk(4).await.unwrap());
        let mut state = file.0.lock().await;
        let AsyncFileState::Writer(sink) = &mut *state else {
            unreachable!();
        };

        // Poll into an underlying write blocked on the semaphore, then drop
        // the caller's future. The sink must retain both the future and input.
        {
            let send = sink.send(buffer);
            futures::pin_mut!(send);
            assert!(poll!(send).is_pending());
        }
        semaphore.release(1);
        sink.close().await.unwrap();
        drop(state);
        drop(file);

        let result = op.read("cancelled").await.unwrap();
        assert_eq!(result.to_vec(), b"owned Python bytes");
        assert_eq!(result.current().as_ptr(), source_ptr);
    }

    struct RuntimeOwnedBytes(Arc<AtomicBool>);

    impl AsRef<[u8]> for RuntimeOwnedBytes {
        fn as_ref(&self) -> &[u8] {
            b"pending input"
        }
    }

    impl Drop for RuntimeOwnedBytes {
        fn drop(&mut self) {
            self.0.store(
                tokio::runtime::Handle::try_current().is_ok(),
                Ordering::SeqCst,
            );
        }
    }

    #[test]
    fn test_unclosed_file_drops_pending_input_in_runtime() {
        let op = ocore::Operator::via_iter("memory", []).unwrap();
        let writer = get_runtime()
            .block_on(async { op.writer_with("pending").chunk(256 * 1024).await })
            .unwrap();
        let dropped_in_runtime = Arc::new(AtomicBool::new(false));
        let data = bytes::Bytes::from_owner(RuntimeOwnedBytes(dropped_in_runtime.clone()));
        let mut file = File::new_writer(writer);
        let FileState::Writer(sink) = &mut file.0 else {
            unreachable!();
        };
        get_runtime().block_on(sink.send(data.into())).unwrap();
        assert!(!dropped_in_runtime.load(Ordering::SeqCst));
        assert!(tokio::runtime::Handle::try_current().is_err());
        drop(file);
        assert!(dropped_in_runtime.load(Ordering::SeqCst));
    }
}
