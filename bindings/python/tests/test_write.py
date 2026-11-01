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

import gc
import os
import sys
from pathlib import Path
from random import randint
from uuid import uuid4

import pytest

import opendal
from opendal.exceptions import NotFound


@pytest.mark.need_capability("write", "delete", "stat")
def test_sync_write(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    size = len(content)
    operator.write(filename, content, content_type="text/plain")
    metadata = operator.stat(filename)
    assert metadata is not None
    assert metadata.mode.is_file()
    assert metadata.content_length == size

    operator.delete(filename)


@pytest.mark.need_capability("write", "delete", "stat")
def test_sync_write_path(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = Path(f"test_file_{str(uuid4())}.txt")
    content = os.urandom(size)
    size = len(content)
    operator.write(filename, content, content_type="text/plain")
    metadata = operator.stat(filename)
    assert metadata is not None
    assert metadata.mode.is_file()
    assert metadata.content_length == size

    operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("write", "delete", "stat")
async def test_async_write(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    size = len(content)
    await async_operator.write(filename, content)
    metadata = await async_operator.stat(filename)
    assert metadata is not None
    assert metadata.mode.is_file()
    assert metadata.content_length == size

    await async_operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("write", "delete", "stat")
async def test_async_write_path(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = Path(f"test_file_{str(uuid4())}.txt")
    content = os.urandom(size)
    size = len(content)
    await async_operator.write(filename, content)
    metadata = await async_operator.stat(filename)
    assert metadata is not None
    assert metadata.mode.is_file()
    assert metadata.content_length == size

    await async_operator.delete(filename)


@pytest.mark.need_capability("create_dir", "stat")
def test_sync_create_dir(service_name, operator, async_operator):
    path = f"test_dir_{str(uuid4())}/"
    operator.create_dir(path)
    metadata = operator.stat(path)
    assert metadata is not None
    assert metadata.mode.is_dir()

    operator.delete(path)


@pytest.mark.asyncio
@pytest.mark.need_capability("create_dir", "stat")
async def test_async_create_dir(service_name, operator, async_operator):
    path = f"test_dir_{str(uuid4())}/"
    await async_operator.create_dir(path)
    metadata = await async_operator.stat(path)
    assert metadata is not None
    assert metadata.mode.is_dir()

    await async_operator.delete(path)


@pytest.mark.need_capability("delete", "stat")
def test_sync_delete(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    size = len(content)
    operator.write(filename, content)
    operator.delete(filename)
    with pytest.raises(NotFound):
        operator.stat(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("delete", "stat")
async def test_async_delete(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    size = len(content)
    await async_operator.write(filename, content)
    await async_operator.delete(filename)
    with pytest.raises(NotFound):
        await async_operator.stat(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("write", "delete")
async def test_async_writer(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    f = await async_operator.open(filename, "wb")
    written_bytes = await f.write(content)
    assert written_bytes == size
    await f.close()
    await async_operator.delete(filename)
    with pytest.raises(NotFound):
        await async_operator.stat(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("write", "read", "delete")
async def test_async_writer_keeps_bytes_alive(service_name, operator, async_operator):
    filename = f"test_file_{str(uuid4())}.txt"
    expected = bytes(range(256)) * 4
    content = bytes(bytearray(expected))
    f = await async_operator.open(filename, "wb")

    write = f.write(content)
    del content
    gc.collect()

    written_bytes = await write
    assert written_bytes == len(expected)
    await f.close()
    assert await async_operator.read(filename) == expected
    await async_operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("write", "write_can_multi", "read", "delete")
@pytest.mark.parametrize("chunk", [None, 256 * 1024, 8 * 1024 * 1024])
async def test_async_writer_mixed_sizes(async_operator, chunk):
    filename = f"test_file_{uuid4()}"
    sizes = [0, 1, 17, 256 * 1024, 256 * 1024 + 1, 8 * 1024 * 1024, 31, 0]
    expected = bytearray()
    options = {} if chunk is None else {"chunk": chunk}
    async with await async_operator.open(filename, "wb", **options) as file:
        for i, size in enumerate(sizes):
            content = bytes([i]) * size
            expected.extend(content)
            assert await file.write(content) == size
            del content
        gc.collect()
    assert await file.closed
    await file.close()
    with pytest.raises(OSError, match="closed file"):
        await file.write(b"")
    assert await async_operator.read(filename) == expected
    await async_operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.skipif(not hasattr(sys, "getrefcount"), reason="requires reference counts")
@pytest.mark.parametrize("size", [17, 256 * 1024, 8 * 1024 * 1024 + 1])
async def test_async_writer_retains_python_owner(size):
    op = opendal.AsyncOperator("memory")
    content = os.urandom(size)
    references = sys.getrefcount(content)
    file = await op.open("owner", "wb", chunk=8 * 1024 * 1024)
    assert await file.write(content) == size
    # A copied staging buffer would let the Python owner go after write returns.
    assert sys.getrefcount(content) > references
    await file.close()
    assert await op.read("owner") == content
    await op.delete("owner")
    gc.collect()
    assert sys.getrefcount(content) == references


@pytest.mark.asyncio
@pytest.mark.need_capability("write", "delete", "write_with_if_not_exists")
async def test_async_writer_options(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    f = await async_operator.open(filename, "wb")
    written_bytes = await f.write(content)
    assert written_bytes == size
    await f.close()

    with pytest.raises(Exception) as excinfo:  # noqa PT011 PT012
        async with await async_operator.open(filename, "wb", if_not_exists=True) as w:
            w.write(content)
        assert "ConditionNotMatch" in str(excinfo.value)


@pytest.mark.need_capability("write", "delete")
def test_sync_writer(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    f = operator.open(filename, "wb")
    written_bytes = f.write(content)
    assert written_bytes == size
    f.close()
    operator.delete(filename)
    with pytest.raises(NotFound):
        operator.stat(filename)


@pytest.mark.need_capability("write", "delete", "write_with_if_not_exists")
def test_sync_writer_options(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    f = operator.open(filename, "wb")
    written_bytes = f.write(content)
    assert written_bytes == size
    f.close()

    with pytest.raises(Exception) as excinfo:  # noqa PT011 PT012
        with operator.open(filename, "wb", if_not_exists=True) as w:
            w.write(content)
        assert "ConditionNotMatch" in str(excinfo.value)


@pytest.mark.need_capability("write", "write_can_multi", "read", "delete")
@pytest.mark.parametrize(
    "sizes",
    [
        [0],
        [1] * 20,
        [256 * 1024 - 1, 1, 256 * 1024, 256 * 1024 + 1],
        [8 * 1024 * 1024],
        [1, 8 * 1024 * 1024 + 7, 3, 256 * 1024],
    ],
)
@pytest.mark.parametrize("flush", [False, True])
@pytest.mark.parametrize("chunk", [None, 8 * 1024 * 1024])
def test_sync_writer_owned_buffers(operator, sizes, flush, chunk):
    if not any(sizes) and not operator.capability().write_can_empty:
        pytest.skip("empty writes are not supported")
    filename = f"test_owned_{uuid4()}"
    expected = bytearray()
    try:
        options = {} if chunk is None else {"chunk": chunk}
        with operator.open(filename, "wb", **options) as file:
            for size in sizes:
                content = os.urandom(size)
                expected.extend(content)
                assert file.write(content) == size
                del content
                if flush:
                    file.flush()
                    file.flush()
        assert file.closed
        assert operator.read(filename) == expected
        with pytest.raises(OSError, match="closed file"):
            file.write(b"")
    finally:
        operator.delete(filename)


@pytest.mark.skipif(not hasattr(sys, "getrefcount"), reason="requires reference counts")
@pytest.mark.parametrize("size", [17, 256 * 1024, 8 * 1024 * 1024 + 7])
def test_sync_writer_retains_bytes(service_name, size):
    if service_name != "memory":
        pytest.skip("inspect ownership with the memory service")
    op = opendal.Operator("memory")
    content = os.urandom(size)
    references = sys.getrefcount(content)
    with op.open("owned", "wb", chunk=8 * 1024 * 1024) as file:
        assert file.write(content) == len(content)
        assert sys.getrefcount(content) > references
    assert op.read("owned") == content
    op.delete("owned")
    assert sys.getrefcount(content) == references


@pytest.mark.skipif(not hasattr(sys, "getrefcount"), reason="requires reference counts")
def test_sync_writer_drop_releases_pending_owner(service_name):
    if service_name != "memory":
        pytest.skip("inspect ownership with the memory service")
    op = opendal.Operator("memory")
    content = os.urandom(17)
    references = sys.getrefcount(content)
    file = op.open("unclosed", "wb")
    file.write(content)
    assert sys.getrefcount(content) > references
    del file
    gc.collect()
    assert sys.getrefcount(content) == references
    assert not op.exists("unclosed")


@pytest.mark.need_capability("write", "read", "delete")
def test_sync_writer_preserves_bytes_only_input(operator):
    filename = f"test_input_{uuid4()}"
    expected = b"valid bytes"
    with operator.open(filename, "wb") as file:
        for content in (bytearray(b"mutable"), memoryview(b"view"), [1, 2]):
            with pytest.raises(TypeError):
                file.write(content)
        assert file.write(expected) == len(expected)
    assert operator.read(filename) == expected
    operator.delete(filename)
