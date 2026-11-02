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

import asyncio
from uuid import uuid4

import pytest

import opendal


@pytest.mark.asyncio
async def test_async_list_batching_preserves_order():
    op = opendal.AsyncOperator("memory")
    expected = [f"items/{idx:03d}" for idx in range(130)]
    await asyncio.gather(*(op.write(path, b"test") for path in expected))

    actual = [entry.path async for entry in await op.list("items/", recursive=True)]
    assert actual == expected


@pytest.mark.asyncio
async def test_async_list_cancelled_next_preserves_entry():
    op = opendal.AsyncOperator("memory")
    expected = [f"items/{idx:03d}" for idx in range(3)]
    await asyncio.gather(*(op.write(path, b"test") for path in expected))

    lister = await op.list("items/", recursive=True)
    pending = anext(lister)
    pending.cancel()
    with pytest.raises(asyncio.CancelledError):
        await pending

    actual = [entry.path async for entry in lister]
    assert actual == expected


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "copy", "list", "list_with_start_after")
async def test_async_list_with_start_after(service_name, operator, async_operator):
    test_dir = f"test_async_list_dir_{uuid4()}/"
    await async_operator.create_dir(test_dir)

    # 1. Prepare data
    files_to_create = [f"{test_dir}file_{i}" for i in range(5)]
    # Create files concurrently
    await asyncio.gather(
        *(async_operator.write(f, b"test_content") for f in files_to_create)
    )

    # 2. Test basic list
    entries = sorted(
        [entry.path async for entry in await async_operator.list(test_dir)]
    )  # Ensure order for comparison
    expected_files = sorted([test_dir, *files_to_create])
    assert entries == expected_files, (
        f"Basic list failed. Expected {expected_files}, got {entries}"
    )

    # 3. Test list with start_after
    start_after_file = files_to_create[2]  # e.g., test_dir/file_2
    # Note: start_after expects the *full path* relative to the operator root
    entries_after = sorted(
        [
            entry.path
            async for entry in await async_operator.list(
                test_dir, start_after=start_after_file
            )
        ]
    )

    # Expected files are those lexicographically after start_after_file
    expected_files_after = sorted([f for f in files_to_create if f > start_after_file])
    assert entries_after == expected_files_after, (
        f"Expected {expected_files_after} after {start_after_file}, got {entries_after}"
    )
    # 6. Cleanup
    await async_operator.remove_all(test_dir)
