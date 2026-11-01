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


@pytest.mark.need_capability("write", "stat", "stat_with_if_match")
def test_stat_accepts_if_match_param(service_name, operator, async_operator):
    path = f"test_stat_if_match_{uuid4()}.txt"
    operator.write(path, b"test content")
    meta = operator.stat(path)
    etag = meta.etag
    if etag is None:
        pytest.skip("backend does not return etag")
    operator.stat(path, if_match=etag)


@pytest.mark.need_capability("write", "stat", "stat_with_if_none_match")
def test_stat_accepts_if_none_match_param(service_name, operator, async_operator):
    path = f"test_stat_if_none_match_{uuid4()}.txt"
    operator.write(path, b"test content")
    operator.stat(path, if_none_match="etag456")


@pytest.mark.need_capability("write", "stat", "stat_with_version")
def test_stat_accepts_version_param(service_name, operator, async_operator):
    path = f"test_stat_version_{uuid4()}.txt"
    operator.write(path, b"test content")
    meta = operator.stat(path)
    if meta.version is None:
        pytest.skip("backend does not return version")
    operator.stat(path, version=meta.version)


@pytest.mark.need_capability("write", "stat")
def test_stat_default_params_unchanged(service_name, operator, async_operator):
    path = f"test_stat_default_{uuid4()}.txt"
    operator.write(path, b"test content")
    meta = operator.stat(path)
    assert meta.content_length == len(b"test content")


@pytest.mark.need_capability("write", "stat", "stat_with_if_match")
@pytest.mark.asyncio
async def test_async_stat_accepts_if_match_param(
    service_name, operator, async_operator
):
    path = f"test_async_stat_if_match_{uuid4()}.txt"
    await async_operator.write(path, b"test content")
    meta = await async_operator.stat(path)
    etag = meta.etag
    if etag is None:
        pytest.skip("backend does not return etag")
    await async_operator.stat(path, if_match=etag)


@pytest.mark.need_capability("write", "stat", "stat_with_if_none_match")
@pytest.mark.asyncio
async def test_async_stat_accepts_if_none_match_param(
    service_name, operator, async_operator
):
    path = f"test_async_stat_if_none_match_{uuid4()}.txt"
    await async_operator.write(path, b"test content")
    await async_operator.stat(path, if_none_match="etag456")


@pytest.mark.need_capability("write", "stat", "stat_with_version")
@pytest.mark.asyncio
async def test_async_stat_accepts_version_param(service_name, operator, async_operator):
    path = f"test_async_stat_version_{uuid4()}.txt"
    await async_operator.write(path, b"test content")
    meta = await async_operator.stat(path)
    if meta.version is None:
        pytest.skip("backend does not return version")
    await async_operator.stat(path, version=meta.version)
