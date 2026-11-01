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


@pytest.mark.need_capability("read", "write", "copy", "list", "list_with_start_after")
def test_sync_list_with_start_after(service_name, operator, async_operator):
    test_dir = f"test_sync_list_dir_{uuid4()}/"
    operator.create_dir(test_dir)

    # 1. Prepare data
    files_to_create = [f"{test_dir}file_{i}" for i in range(5)]
    for f in files_to_create:
        operator.write(f, b"test_content")

    # 2. Test basic list
    entries = sorted(
        [entry.path for entry in operator.list(test_dir)]
    )  # Ensure order for comparison
    expected_files = sorted([test_dir, *files_to_create])
    assert entries == expected_files, (
        f"Basic list failed. Expected {expected_files}, got {entries}"
    )

    # 3. Test list with start_after
    start_after_file = files_to_create[2]  # e.g., test_dir/file_2
    entries_after = []
    # Note: start_after expects the *full path* relative to the operator root
    entries_after = sorted(
        [entry.path for entry in operator.list(test_dir, start_after=start_after_file)]
    )  # Ensure order

    # Expected files are those lexicographically after start_after_file
    expected_files_after = sorted([f for f in files_to_create if f > start_after_file])
    assert entries_after == expected_files_after, (
        f"Expected {expected_files_after} after {start_after_file}, got {entries_after}"
    )
    # 6. Cleanup
    operator.remove_all(test_dir)
