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

import pytest

from opendal.exceptions import NotFound


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete", "stat")
async def test_async_stat_file(service_name, operator, async_operator):
    path = f"test_async_stat_{uuid4()}.txt"
    content = os.urandom(1024)
    await async_operator.write(path, content)

    meta = await async_operator.stat(path)
    assert meta is not None
    assert meta.content_length == len(content)
    assert meta.is_file
    assert not meta.is_dir

    await async_operator.delete(path)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete", "stat")
async def test_async_stat_non_existent(service_name, operator, async_operator):
    path = f"test_async_stat_non_existent_{uuid4()}.txt"
    with pytest.raises(NotFound):
        await async_operator.stat(path)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete", "stat", "create_dir")
async def test_async_stat_dir(service_name, operator, async_operator):
    path = f"test_async_stat_dir_{uuid4()}/"
    await async_operator.create_dir(path)

    meta = await async_operator.stat(path)
    assert meta is not None
    assert meta.is_dir
    assert not meta.is_file

    await async_operator.delete(path)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete", "stat")
async def test_async_stat_metadata_fields(service_name, operator, async_operator):
    path = f"test_async_stat_meta_{uuid4()}.txt"
    content = b"hello world"
    await async_operator.write(path, content)

    meta = await async_operator.stat(path)
    assert meta.content_length == len(content)
    # etag and content_type may or may not be present depending on backend
    assert meta.etag is not None or meta.etag is None
    assert meta.content_type is not None or meta.content_type is None

    await async_operator.delete(path)
