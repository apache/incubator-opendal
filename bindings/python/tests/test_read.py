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

import opendal
import pytest


def test_sync_read(operator: opendal.Operator):
    filename = f'random_file_{str(uuid4())}'
    content = b'w' * 1024
    operator.write(filename, content)

    read_content = operator.read(filename)
    assert read_content is not None
    assert read_content == content


def test_sync_read_stat(operator: opendal.Operator):
    filename = f'random_file_{str(uuid4())}'
    content = b'w' * 1024
    operator.write(filename, content)

    metadata = operator.stat(filename)
    assert metadata is not None
    assert metadata.content_length == len(content)
    assert metadata.mode.is_file()


def test_sync_read_not_exists(operator: opendal.Operator):
    with pytest.raises(FileNotFoundError):
        operator.read(str(uuid4()))
