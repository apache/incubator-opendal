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

import io
import os
from datetime import timedelta
from pathlib import Path
from random import choices, randint
from uuid import uuid4

import pytest

from opendal.exceptions import ConditionNotMatch, NotFound


@pytest.mark.need_capability("read", "write", "delete")
def test_sync_read(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"random_file_{str(uuid4())}"
    content = os.urandom(size)
    operator.write(filename, content)

    read_content = operator.read(filename)
    assert read_content is not None
    assert read_content == content

    operator.delete(filename)


@pytest.mark.need_capability("read", "write", "delete")
def test_sync_reader(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"random_file_{str(uuid4())}"
    content = os.urandom(size)
    operator.write(filename, content)

    with operator.open(filename, "rb") as reader:
        assert reader.readable()
        assert not reader.writable()
        assert not reader.closed

        read_content = reader.read()
        assert read_content is not None
        assert read_content == content

    with operator.open(filename, "rb") as reader:
        read_content = bytearray()
        while True:
            chunk = reader.read(size + 1)
            if not chunk:
                break
            read_content.extend(chunk)

        read_content = bytes(read_content)
        assert read_content is not None
        assert read_content == content

    buf = bytearray(1)
    with operator.open(filename, "rb") as reader:
        reader.readinto(buf)
        assert buf == content[:1]

    range_start = randint(0, len(content) - 1)
    range_end = randint(range_start, len(content) - 1)

    with operator.open(
        filename, "rb", offset=range_start, size=range_end - range_start
    ) as reader:
        assert reader.readable()
        assert not reader.writable()
        assert not reader.closed

        read_content = reader.read()
        assert read_content is not None
        assert read_content == content[range_start:range_end]

    operator.delete(filename)


@pytest.mark.need_capability("read", "write", "delete")
def test_sync_reader_read_sizes(operator):
    filename = f"random_file_{str(uuid4())}"
    content = b"hello, world"
    operator.write(filename, content)

    with operator.open(filename, "rb") as reader:
        assert reader.read(0) == b""
        assert reader.tell() == 0
        assert reader.read(5) == b"hello"
        assert reader.tell() == 5
        assert reader.read(1024) == b", world"
        assert reader.read(1) == b""
        reader.seek(2)
        assert reader.read() == b"llo, world"

    operator.delete(filename)


@pytest.mark.need_capability("read", "write", "delete")
def test_sync_reader_readline(service_name, operator, async_operator):
    size = randint(1, 1024)
    lines = randint(1, min(100, size))
    filename = f"random_file_{str(uuid4())}"
    content = bytearray(os.urandom(size))

    for idx in choices(range(size), k=lines):
        content[idx] = ord("\n")
    operator.write(filename, content)

    line_contents = io.BytesIO(content).readlines()
    i = 0

    with operator.open(filename, "rb") as reader:
        assert reader.readable()
        assert not reader.writable()
        assert not reader.closed

        while (read_content := reader.readline()) != b"":
            assert read_content is not None
            assert read_content == line_contents[i]
            i += 1

    operator.delete(filename)


@pytest.mark.need_capability("read", "write", "delete")
def test_sync_reader_readline_with_size(operator):
    filename = f"random_file_{str(uuid4())}"
    content = b"alpha\nbeta\ngamma"
    operator.write(filename, content)

    expected = io.BytesIO(content)
    with operator.open(filename, "rb") as reader:
        for size in [3, 10, 0, 5, 2, 10, 10]:
            assert reader.readline(size) == expected.readline(size)

    operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete")
async def test_async_read(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"random_file_{str(uuid4())}"
    content = os.urandom(size)
    await async_operator.write(filename, content)

    read_content = await async_operator.read(filename)
    assert read_content is not None
    assert read_content == content

    await async_operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete")
async def test_async_read_path(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = Path(f"random_file_{str(uuid4())}")
    content = os.urandom(size)
    await async_operator.write(filename, content)

    read_content = await async_operator.read(filename)
    assert read_content is not None
    assert read_content == content

    await async_operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete")
async def test_async_reader(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"random_file_{str(uuid4())}"
    content = os.urandom(size)
    await async_operator.write(filename, content)

    async with await async_operator.open(filename, "rb") as reader:
        assert await reader.readable()
        assert not await reader.writable()
        assert not await reader.closed

        read_content = await reader.read()
        assert read_content is not None
        assert read_content == content

    async with await async_operator.open(filename, "rb") as reader:
        read_content = bytearray()
        while True:
            chunk = await reader.read(size + 1)
            if not chunk:
                break
            read_content.extend(chunk)

        read_content = bytes(read_content)
        assert read_content is not None
        assert read_content == content

    range_start = randint(0, len(content) - 1)
    range_end = randint(range_start, len(content) - 1)

    async with await async_operator.open(
        filename, "rb", offset=range_start, size=range_end - range_start
    ) as reader:
        assert await reader.readable()
        assert not await reader.writable()
        assert not await reader.closed

        read_content = await reader.read()
        assert read_content is not None
        assert read_content == content[range_start:range_end]

    await async_operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete")
async def test_async_reader_read_sizes(async_operator):
    filename = f"random_file_{str(uuid4())}"
    content = b"hello, world"
    await async_operator.write(filename, content)

    async with await async_operator.open(filename, "rb") as reader:
        assert await reader.read(0) == b""
        assert await reader.tell() == 0
        assert await reader.read(5) == b"hello"
        assert await reader.tell() == 5
        assert await reader.read(1024) == b", world"
        assert await reader.read(1) == b""
        await reader.seek(2)
        assert await reader.read() == b"llo, world"

    await async_operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete")
async def test_async_reader_without_context(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"random_file_{str(uuid4())}"
    content = os.urandom(size)
    await async_operator.write(filename, content)

    reader = await async_operator.open(filename, "rb")
    read_content = await reader.read()
    assert read_content is not None
    assert read_content == content
    await reader.close()

    await async_operator.delete(filename)


@pytest.mark.need_capability("read", "write", "delete", "stat")
def test_sync_read_stat(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"random_file_{str(uuid4())}"
    content = os.urandom(size)
    operator.write(filename, content)

    metadata = operator.stat(filename)
    assert metadata is not None
    assert metadata.content_length == len(content)
    assert metadata.mode.is_file()

    operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete", "stat")
async def test_async_read_stat(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"random_file_{str(uuid4())}"
    content = os.urandom(size)
    await async_operator.write(filename, content)

    metadata = await async_operator.stat(filename)
    assert metadata is not None
    assert metadata.content_length == len(content)
    assert metadata.mode.is_file()

    await async_operator.delete(filename)

    operator.delete(filename)


@pytest.mark.need_capability("read")
def test_sync_read_not_exists(service_name, operator, async_operator):
    with pytest.raises(NotFound):
        operator.read(str(uuid4()))


@pytest.mark.asyncio
@pytest.mark.need_capability("read")
async def test_async_read_not_exists(service_name, operator, async_operator):
    with pytest.raises(NotFound):
        await async_operator.read(str(uuid4()))


@pytest.mark.need_capability(
    "read",
    "write",
    "read_with_if_modified_since",
    "read_with_if_unmodified_since",
)
def test_sync_conditional_reads(service_name, operator):
    path = f"random_file_{str(uuid4())}"
    content = b"test data"
    operator.write(path, content)

    metadata = operator.stat(path)
    mod_time = metadata.last_modified
    assert mod_time is not None

    # Large delta: 1 minute earlier
    before = mod_time - timedelta(minutes=1)
    after = mod_time + timedelta(seconds=10)

    # Should succeed: file was modified after `before`
    assert operator.read(path, if_modified_since=before) == content

    # Should succeed: file was unmodified since `after`
    assert operator.read(path, if_unmodified_since=after) == content

    # Should fail: file was modified after `before`
    with pytest.raises(ConditionNotMatch):
        operator.read(path, if_unmodified_since=before)


@pytest.mark.need_capability("read", "write", "delete")
def test_sync_readinto_buffer_boundaries(operator):
    filename = f"readinto_{uuid4()}"
    operator.write(filename, b"abcdef")
    try:
        with operator.open(filename, "rb") as reader:
            with pytest.raises(OSError, match="not writable"):
                reader.readinto(b"readonly")
            with pytest.raises(OSError, match="not C contiguous"):
                reader.readinto(memoryview(bytearray(8))[::2])
            assert reader.tell() == 0
            assert reader.readinto(bytearray()) == 0
            assert reader.tell() == 0
            target = bytearray(b"??????????")
            assert reader.readinto(memoryview(target)[2:5]) == 3
            assert target == b"??abc?????"
            assert reader.tell() == 3
            assert reader.readinto(memoryview(target)[5:]) == 3
            assert target == b"??abcdef??"
            assert reader.tell() == 6
            assert reader.readinto(target) == 0
            assert target == b"??abcdef??"
    finally:
        operator.delete(filename)
