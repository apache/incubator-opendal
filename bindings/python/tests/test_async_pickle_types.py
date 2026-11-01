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
import pickle
from random import randint
from uuid import uuid4

import pytest


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete", "shared")
async def test_operator_pickle(service_name, operator, async_operator):
    """Test AsyncOperator's pickle serialization and deserialization."""
    size = randint(1, 1024)
    filename = f"random_file_{str(uuid4())}"
    content = os.urandom(size)
    await async_operator.write(filename, content)

    serialized = pickle.dumps(async_operator)

    deserialized = pickle.loads(serialized)
    assert await deserialized.read(filename) == content

    await async_operator.delete(filename)
