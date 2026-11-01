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

from opendal.exceptions import NotFound


@pytest.mark.asyncio
@pytest.mark.need_capability("read", "write", "delete", "list", "create_dir")
async def test_async_remove_all(service_name, operator, async_operator):
    parent = f"random_dir_{str(uuid4())}"
    excepted = [
        "x/",
        "x/y",
        "x/x/",
        "x/x/y",
        "x/x/x/",
        "x/x/x/y",
        "x/x/x/x/",
    ]
    for path in excepted:
        if path.endswith("/"):
            await async_operator.create_dir(f"{parent}/{path}")
        else:
            await async_operator.write(f"{parent}/{path}", os.urandom(1024))
    await async_operator.remove_all(f"{parent}/x/")
    for path in excepted:
        if not path.endswith("/"):
            with pytest.raises(NotFound):
                await async_operator.read(f"{parent}/{path}")
    await async_operator.remove_all(f"{parent}/")
