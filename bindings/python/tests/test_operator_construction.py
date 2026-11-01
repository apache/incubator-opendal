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

import pytest

import opendal


@pytest.mark.parametrize("path_like", [False, True])
def test_operator_new_with_path(tmp_path, path_like):
    root = tmp_path if path_like else str(tmp_path)
    op = opendal.Operator("fs", root=root)
    op.write("constructor.txt", b"sync constructor")
    assert op.read("constructor.txt") == b"sync constructor"
    assert (tmp_path / "constructor.txt").read_bytes() == b"sync constructor"


@pytest.mark.asyncio
@pytest.mark.parametrize("path_like", [False, True])
async def test_async_operator_new_with_path(tmp_path, path_like):
    root = tmp_path if path_like else str(tmp_path)
    op = opendal.AsyncOperator("fs", root=root)
    await op.write("constructor.txt", b"async constructor")
    assert await op.read("constructor.txt") == b"async constructor"
    assert (tmp_path / "constructor.txt").read_bytes() == b"async constructor"


@pytest.mark.parametrize("operator_type", [opendal.Operator, opendal.AsyncOperator])
@pytest.mark.parametrize("root", [True, b"bytes-root"])
def test_constructor_rejects_non_string_root(operator_type, root):
    with pytest.raises(TypeError):
        operator_type("fs", root=root)
