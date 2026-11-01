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


@pytest.mark.need_capability(
    "write", "delete", "delete_with_version", "stat_with_version"
)
def test_delete_accepts_version_param(service_name, operator, async_operator):
    path = f"test_delete_version_{uuid4()}.txt"
    operator.write(path, b"test content")
    meta = operator.stat(path)
    if meta.version is None:
        pytest.skip("backend does not return version")
    operator.delete(path, version=meta.version)


@pytest.mark.need_capability("write", "delete", "delete_with_recursive")
def test_delete_accepts_recursive_param(service_name, operator, async_operator):
    path = f"test_delete_recursive_{uuid4()}.txt"
    operator.write(path, b"test content")
    operator.delete(path, recursive=True)


@pytest.mark.need_capability("write", "delete")
def test_delete_default_params_unchanged(service_name, operator, async_operator):
    path = f"test_delete_default_{uuid4()}.txt"
    operator.write(path, b"test content")
    operator.delete(path)


@pytest.mark.need_capability(
    "write", "delete", "delete_with_version", "stat_with_version"
)
@pytest.mark.asyncio
async def test_async_delete_accepts_version_param(
    service_name, operator, async_operator
):
    path = f"test_async_delete_version_{uuid4()}.txt"
    await async_operator.write(path, b"test content")
    meta = await async_operator.stat(path)
    if meta.version is None:
        pytest.skip("backend does not return version")
    await async_operator.delete(path, version=meta.version)


@pytest.mark.need_capability("write", "delete", "delete_with_recursive")
@pytest.mark.asyncio
async def test_async_delete_accepts_recursive_param(
    service_name, operator, async_operator
):
    path = f"test_async_delete_recursive_{uuid4()}.txt"
    await async_operator.write(path, b"test content")
    await async_operator.delete(path, recursive=True)
