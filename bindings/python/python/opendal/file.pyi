# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

import collections.abc
import types
from typing import Any, final

import typing_extensions

@final
class AsyncFile:
    """
    An async file-like object for reading and writing data.

    Created by the `open` method of the `AsyncOperator` class.
    """

    def __aenter__(self, /) -> collections.abc.Awaitable[typing_extensions.Self]: ...
    def __aexit__(
        self,
        /,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: types.TracebackType | None,
    ) -> None: ...
    def close(self, /) -> collections.abc.Awaitable[None]:
        """
        Close this file.

        This also flushes write buffers, if applicable.

        Notes
        -----
        A closed file cannot be used for further I/O operations.
        """
    @property
    def closed(self, /) -> Any:
        """
        Whether this file is closed.

        Returns
        -------
        coroutine
            An awaitable that returns True if this file is closed.
        """
    def read(self, /, size: int | None = None) -> collections.abc.Awaitable[bytes]:
        """
        Read at most `size` bytes from this file asynchronously.

        If `size` is not specified, read until EOF.

        Parameters
        ----------
        size : int, optional
            The maximum number of bytes to read.

        Notes
        -----
        Fewer bytes may be returned than requested, read in a loop
        to ensure all bytes are read.

        Returns
        -------
        coroutine
            An awaitable that returns the bytes read from the stream.
        """
    def readable(self, /) -> collections.abc.Awaitable[bool]:
        """
        Whether this file can be read from.

        Returns
        -------
        coroutine
            An awaitable that returns True if this file can be read from.
        """
    def seek(self, /, pos: int, whence: int = 0) -> collections.abc.Awaitable[int]:
        """
        Change the position of this file to the given byte offset.

        Parameters
        ----------
        pos : int
            The byte offset (position) to set.
        whence : int, optional
            The reference point for the offset.
            0: start of file (default); 1: current position; 2: end of file.

        Notes
        -----
        Unbounded readable files allow non-negative positions beyond EOF. Reads
        from those positions return empty bytes when the backing service reports
        an unsatisfied unbounded range. Other service errors propagate. Explicit
        bounded views reject positions beyond their range.

        Returns
        -------
        coroutine
            An awaitable that returns the current absolute position.
        """
    def seekable(self, /) -> collections.abc.Awaitable[bool]:
        """
        Whether this file can be repositioned.

        Notes
        -----
        This is only applicable to *readable* files.

        Returns
        -------
        coroutine
            An awaitable that returns True if this file can be repositioned.
        """
    def tell(self, /) -> collections.abc.Awaitable[int]:
        """
        Return the current position of this file.

        Returns
        -------
        coroutine
            An awaitable that returns the current absolute position.
        """
    def writable(self, /) -> collections.abc.Awaitable[bool]:
        """
        Whether this file can be written to.

        Returns
        -------
        coroutine
            An awaitable that returns True if this file can be written to.
        """
    def write(self, /, bs: bytes) -> collections.abc.Awaitable[int]:
        """
        Write bytes to this file asynchronously.

        Parameters
        ----------
        bs : bytes
            The bytes to write to the file.

        Returns
        -------
        coroutine
            An awaitable that returns the number of bytes written.
        """

@final
class File:
    """
    A file-like object for reading and writing data.

    Created by the `open` method of the `Operator` class.

    Storage I/O releases the Python GIL while waiting. Concurrent operations on
    the same file are rejected; use separate files for I/O from multiple threads.
    """

    def __enter__(self, /) -> File: ...
    def __exit__(
        self,
        /,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: types.TracebackType | None,
    ) -> None: ...
    def close(self, /) -> None:
        """
        Close this file.

        This also flushes write buffers, if applicable.

        Notes
        -----
        A closed file cannot be used for further I/O operations.
        """
    @property
    def closed(self, /) -> bool:
        """
        Whether this file is closed.

        Returns
        -------
        bool
            True if this file is closed.
        """
    def flush(self, /) -> None:
        """
        Flush the underlying writer.

        Notes
        -----
        Passes pending input to the core writer. Storage chunks can remain
        buffered until more data arrives or `close()` completes the write.
        Is a no-op if the file is not `writable`.
        """
    def read(self, /, size: int | None = None) -> bytes:
        """
        Read at most `size` bytes from this file.

        If `size` is not specified, read until EOF.

        Parameters
        ----------
        size : int, optional
            The maximum number of bytes to read.

        Notes
        -----
        Fewer bytes may be returned than requested, read in a loop
        to ensure all bytes are read.

        Returns
        -------
        bytes
            The bytes read from this file.
        """
    def readable(self, /) -> bool:
        """
        Whether this file can be read from.

        Returns
        -------
        bool
            True if this file can be read from.
        """
    def readinto(self, /, buffer: bytearray | memoryview) -> int:
        """
        Read bytes into a pre-allocated buffer.

        Parameters
        ----------
        buffer : bytes | bytearray
            A writable, pre-allocated buffer to read into.

        Returns
        -------
        int
            The number of bytes read.
        """
    def readline(self, /, size: int | None = None) -> bytes:
        """
        Read one line from this file.

        If `size` is not specified, read until newline.

        Parameters
        ----------
        size : int, optional
            The maximum number of bytes to read.

        Notes
        -----
        Retains newline characters after each line, unless
        the file’s last line has no terminating newline.

        Returns
        -------
        bytes
            The bytes read from this file.
        """
    def seek(self, /, pos: int, whence: int = 0) -> int:
        """
        Change the position of this file to the given byte offset.

        Parameters
        ----------
        pos : int
            The byte offset (position) to set.
        whence : int, optional
            The reference point for the offset.
            0: start of file (default); 1: current position; 2: end of file.

        Notes
        -----
        Unbounded readable files allow non-negative positions beyond EOF. Reads
        from those positions return empty bytes when the backing service reports
        an unsatisfied unbounded range. Other service errors propagate. Explicit
        bounded views reject positions beyond their range.

        Returns
        -------
        int
            The new absolute position.
        """
    def seekable(self, /) -> bool:
        """
        Whether this file can be repositioned.

        Notes
        -----
        This is only applicable to *readable* files.

        Returns
        -------
        bool
            True if this file can be repositioned.
        """
    def tell(self, /) -> int:
        """
        Return the current position of this file.

        Returns
        -------
        int
            The current absolute position.
        """
    def writable(self, /) -> bool:
        """
        Whether this file can be written to.

        Returns
        -------
        bool
            True if this file can be written to.
        """
    def write(self, /, bs: bytes) -> int:
        """
        Write bytes to this file.

        Parameters
        ----------
        bs : bytes
            The bytes to write to the file.

        Returns
        -------
        int
            The number of bytes written.
        """
