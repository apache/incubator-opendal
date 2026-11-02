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

from collections.abc import Iterator
from pathlib import Path
from uuid import uuid4

import pytest

import opendal
from opendal.exceptions import NotFound


class StringPath:
    def __init__(self, path: str) -> None:
        self.path = path

    def __fspath__(self) -> str:
        """Return the string path."""
        return self.path

    def __iter__(self) -> Iterator[str]:
        """Make this path-like scalar iterable to test scalar rejection."""
        return iter((self.path,))


class BytesPath:
    def __fspath__(self) -> bytes:
        """Return bytes to test rejection of PathLike[bytes]."""
        return b"survivor"


def raising_paths(path):
    yield path
    message = "iterator failed"
    raise RuntimeError(message)


BAD_INPUTS = [
    pytest.param(lambda path: path, TypeError, id="scalar-string"),
    pytest.param(Path, TypeError, id="scalar-path"),
    pytest.param(StringPath, TypeError, id="iterable-path-like"),
    pytest.param(lambda path: b"", TypeError, id="scalar-bytes"),
    pytest.param(lambda path: [path, None], TypeError, id="invalid-item"),
    pytest.param(lambda path: [path, path.encode()], TypeError, id="bytes-item"),
    pytest.param(lambda path: [path, BytesPath()], TypeError, id="bytes-path-like"),
    pytest.param(raising_paths, RuntimeError, id="raising-iterator"),
]


@pytest.mark.need_capability("read", "write", "delete")
@pytest.mark.parametrize(
    "container",
    [list, tuple, lambda paths: (path for path in paths)],
    ids=["list", "tuple", "generator"],
)
def test_sync_delete_many(operator, container):
    prefix = f"delete_many_{uuid4().hex}"
    names = [f"{prefix}_{suffix}" for suffix in ("str", "path", "custom")]
    for name in names:
        operator.write(name, b"batch delete")
    paths = [names[0], Path(names[1]), StringPath(names[2]), f"{prefix}_missing"]

    operator.delete_many(container(paths))
    operator.delete_many(container(()))

    for name in names:
        with pytest.raises(NotFound):
            operator.read(name)


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete")
@pytest.mark.parametrize(
    "container",
    [list, tuple, lambda paths: (path for path in paths)],
    ids=["list", "tuple", "generator"],
)
async def test_async_delete_many(async_operator, container):
    prefix = f"delete_many_{uuid4().hex}"
    names = [f"{prefix}_{suffix}" for suffix in ("str", "path", "custom")]
    for name in names:
        await async_operator.write(name, b"batch delete")
    paths = [names[0], Path(names[1]), StringPath(names[2]), f"{prefix}_missing"]

    await async_operator.delete_many(container(paths))
    await async_operator.delete_many(container(()))

    for name in names:
        with pytest.raises(NotFound):
            await async_operator.read(name)


@pytest.mark.parametrize(("make_paths", "error"), BAD_INPUTS)
def test_sync_delete_many_input_error(make_paths, error):
    operator = opendal.Operator("memory")
    operator.write("survivor", b"keep")

    with pytest.raises(error):
        operator.delete_many(make_paths("survivor"))

    assert operator.read("survivor") == b"keep"


@pytest.mark.asyncio
@pytest.mark.parametrize(("make_paths", "error"), BAD_INPUTS)
async def test_async_delete_many_input_error(make_paths, error):
    operator = opendal.AsyncOperator("memory")
    await operator.write("survivor", b"keep")

    with pytest.raises(error):
        await operator.delete_many(make_paths("survivor"))

    assert await operator.read("survivor") == b"keep"
