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

import os
from uuid import uuid4
from random import randint

import pytest


@pytest.mark.need_capability("write", "delete", "stat")
def test_sync_write(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"test_file_{str(uuid4())}.txt"
    content = os.urandom(size)
    size = len(content)
    operator.write(filename, content)
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


@pytest.mark.need_capability("write", "delete", "stat")
def test_sync_write_with_non_ascii_name(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"❌😱中文_{str(uuid4())}.test"
    content = os.urandom(size)
    size = len(content)
    operator.write(filename, content)
    metadata = operator.stat(filename)
    assert metadata is not None
    assert metadata.mode.is_file()
    assert metadata.content_length == size

    operator.delete(filename)


@pytest.mark.asyncio
@pytest.mark.need_capability("write", "delete", "stat")
async def test_async_write_with_non_ascii_name(service_name, operator, async_operator):
    size = randint(1, 1024)
    filename = f"❌😱中文_{str(uuid4())}.test"
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
    with pytest.raises(FileNotFoundError):
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
    with pytest.raises(FileNotFoundError):
        await operator.stat(filename)
