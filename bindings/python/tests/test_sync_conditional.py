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

import contextlib
from uuid import uuid4

import pytest

from opendal.exceptions import ConditionNotMatch


@pytest.mark.need_capability("read", "write", "delete", "read_with_if_match")
def test_sync_read_with_if_match(service_name, operator, async_operator):
    path = f"test_sync_if_match_{uuid4()}.txt"
    content = b"test content"
    operator.write(path, content)

    meta = operator.stat(path)
    etag = meta.etag
    if etag is None:
        pytest.skip("backend does not return etag")

    # Matching etag should succeed
    assert operator.read(path, if_match=etag) == content

    # Non-matching etag should fail
    with pytest.raises(ConditionNotMatch):
        operator.read(path, if_match='"invalid-etag"')

    operator.delete(path)


@pytest.mark.need_capability("read", "write", "delete", "read_with_if_none_match")
def test_sync_read_with_if_none_match(service_name, operator, async_operator):
    path = f"test_sync_if_none_match_{uuid4()}.txt"
    content = b"test content"
    operator.write(path, content)

    meta = operator.stat(path)
    etag = meta.etag
    if etag is None:
        pytest.skip("backend does not return etag")

    # Matching etag should fail (resource exists)
    with pytest.raises(ConditionNotMatch):
        operator.read(path, if_none_match=etag)

    # Non-matching etag should succeed
    assert operator.read(path, if_none_match='"invalid-etag"') == content

    operator.delete(path)


@pytest.mark.need_capability("write", "delete", "write_with_if_match")
def test_sync_write_with_if_match(service_name, operator, async_operator):
    path = f"test_sync_write_if_match_{uuid4()}.txt"
    content = b"original content"
    operator.write(path, content)

    meta = operator.stat(path)
    etag = meta.etag
    if etag is None:
        pytest.skip("backend does not return etag")

    # Matching etag should allow overwrite
    new_content = b"updated content"
    operator.write(path, new_content, if_match=etag)
    assert operator.read(path) == new_content

    # Non-matching etag should fail
    with pytest.raises(ConditionNotMatch):
        operator.write(path, b"should not write", if_match='"invalid-etag"')

    operator.delete(path)


@pytest.mark.need_capability("write", "delete", "write_with_if_none_match")
def test_sync_write_with_if_none_match(service_name, operator, async_operator):
    path = f"test_sync_write_if_none_match_{uuid4()}.txt"
    content = b"test content"

    # File does not exist, so any if_none_match should succeed
    operator.write(path, content, if_none_match='"*"')
    assert operator.read(path) == content

    # File now exists — backends may either raise ConditionNotMatch or ignore
    # the header (e.g. Azurite). Verify the file is not overwritten if enforced.
    with contextlib.suppress(ConditionNotMatch):
        operator.write(path, b"should not write", if_none_match='"*"')
    read_back = operator.read(path)
    if read_back != content:
        pytest.skip("backend does not enforce if_none_match on existing files")

    operator.delete(path)
