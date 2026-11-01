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

from opendal.exceptions import IsADirectory, IsSameFile, NotFound


@pytest.mark.need_capability("read", "write", "copy")
def test_sync_copy(service_name, operator, async_operator):
    source_path = f"random_file_{str(uuid4())}"
    content = os.urandom(1024)
    operator.write(source_path, content)
    target_path = f"random_file_{str(uuid4())}"
    operator.copy(source_path, target_path)
    read_content = operator.read(target_path)
    assert read_content is not None
    assert read_content == content
    operator.delete(source_path)
    operator.delete(target_path)


@pytest.mark.need_capability("read", "write", "copy")
def test_sync_copy_non_exist(service_name, operator, async_operator):
    source_path = f"random_file_{str(uuid4())}"
    target_path = f"random_file_{str(uuid4())}"
    with pytest.raises(NotFound):
        operator.copy(source_path, target_path)


@pytest.mark.need_capability("read", "write", "copy", "create_dir")
def test_sync_copy_source_directory(service_name, operator, async_operator):
    source_path = f"random_file_{str(uuid4())}/"
    operator.create_dir(source_path)
    target_path = f"random_file_{str(uuid4())}"
    with pytest.raises(IsADirectory):
        operator.copy(source_path, target_path)


@pytest.mark.need_capability("read", "write", "copy", "create_dir")
def test_sync_copy_target_directory(service_name, operator, async_operator):
    source_path = f"random_file_{str(uuid4())}"
    content = os.urandom(1024)
    operator.write(source_path, content)
    target_path = f"random_file_{str(uuid4())}/"
    operator.create_dir(target_path)
    with pytest.raises(IsADirectory):
        operator.copy(source_path, target_path)
    operator.delete(source_path)
    operator.delete(target_path)


@pytest.mark.need_capability("read", "write", "copy")
def test_sync_copy_self(service_name, operator, async_operator):
    source_path = f"random_file_{str(uuid4())}"
    content = os.urandom(1024)
    operator.write(source_path, content)
    with pytest.raises(IsSameFile):
        operator.copy(source_path, source_path)
    operator.delete(source_path)


@pytest.mark.need_capability("read", "write", "copy")
def test_sync_copy_nested(service_name, operator, async_operator):
    source_path = f"random_file_{str(uuid4())}"
    target_path = f"random_file_{str(uuid4())}/{str(uuid4())}/{str(uuid4())}"
    content = os.urandom(1024)
    operator.write(source_path, content)
    operator.copy(source_path, target_path)
    target_content = operator.read(target_path)
    assert target_content is not None
    assert target_content == content
    operator.delete(source_path)
    operator.delete(target_path)


@pytest.mark.need_capability("read", "write", "copy")
def test_sync_copy_overwrite(service_name, operator, async_operator):
    source_path = f"random_file_{str(uuid4())}"
    target_path = f"random_file_{str(uuid4())}"
    source_content = os.urandom(1024)
    target_content = os.urandom(1024)
    assert source_content != target_content
    operator.write(source_path, source_content)
    operator.write(target_path, target_content)
    operator.copy(source_path, target_path)
    target_content = operator.read(target_path)
    assert target_content is not None
    assert target_content == source_content
    operator.delete(source_path)
    operator.delete(target_path)
