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

import pickle
from datetime import datetime
from uuid import uuid4

import pytest


@pytest.mark.need_capability("read", "write", "delete")
def test_sync_file_pickle(service_name, operator, async_operator):
    """Test pickle streaming serialization and deserialization using operator."""
    data = {
        "a": 1,
        "b": "hello",
        "c": [1, 2, 3],
        "d": {"e": 4},
        "f": None,
        "g": b"hello\nworld",
        "h": {1, 2, 3},
        "i": 1.23,
        "j": True,
        "k": datetime.strptime("2024-01-01", "%Y-%m-%d"),
    }

    filename = f"random_file_{str(uuid4())}"
    with operator.open(filename, "wb") as f:
        pickle.dump(data, f)

    with operator.open(filename, "rb") as f:
        assert pickle.load(f) == data
