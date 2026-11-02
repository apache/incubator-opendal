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

from uuid import uuid4

import pytest


@pytest.mark.need_capability("write", "stat", "write_with_if_match")
def test_write_accepts_if_match_param(service_name, operator, async_operator):
    path = f"test_write_if_match_{uuid4()}.txt"
    operator.write(path, b"initial content")
    etag = operator.stat(path).etag
    if etag is None:
        pytest.skip("backend does not return etag")
    operator.write(path, b"test content", if_match=etag)


@pytest.mark.need_capability("write", "write_with_if_none_match")
def test_write_accepts_if_none_match_param(service_name, operator, async_operator):
    path = f"test_write_if_none_match_{uuid4()}.txt"
    operator.write(path, b"test content", if_none_match="etag456")


@pytest.mark.need_capability("write", "write_with_if_not_exists")
def test_write_accepts_if_not_exists_param(service_name, operator, async_operator):
    path = f"test_write_if_not_exists_{uuid4()}.txt"
    operator.write(path, b"test content", if_not_exists=True)


@pytest.mark.need_capability("write")
def test_write_default_params_unchanged(service_name, operator, async_operator):
    path = f"test_write_default_{uuid4()}.txt"
    operator.write(path, b"test content")


@pytest.mark.need_capability("write", "stat", "write_with_if_match")
@pytest.mark.asyncio
async def test_async_write_accepts_if_match_param(
    service_name, operator, async_operator
):
    path = f"test_async_write_if_match_{uuid4()}.txt"
    await async_operator.write(path, b"initial content")
    meta = await async_operator.stat(path)
    if meta.etag is None:
        pytest.skip("backend does not return etag")
    await async_operator.write(path, b"test content", if_match=meta.etag)


@pytest.mark.need_capability("write", "write_with_if_none_match")
@pytest.mark.asyncio
async def test_async_write_accepts_if_none_match_param(
    service_name, operator, async_operator
):
    path = f"test_async_write_if_none_match_{uuid4()}.txt"
    await async_operator.write(path, b"test content", if_none_match="etag456")


@pytest.mark.need_capability("write", "write_with_if_not_exists")
@pytest.mark.asyncio
async def test_async_write_accepts_if_not_exists_param(
    service_name, operator, async_operator
):
    path = f"test_async_write_if_not_exists_{uuid4()}.txt"
    await async_operator.write(path, b"test content", if_not_exists=True)
