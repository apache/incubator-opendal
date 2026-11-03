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

import pytest

import opendal
from opendal.exceptions import Unsupported


@pytest.mark.need_capability("shared")
def test_from_uri_constructs_configured_service(service_name, setup_config):
    uri_scheme = service_name.replace("_", "-")
    operator = opendal.Operator.from_uri(uri_scheme, **setup_config)
    assert isinstance(operator, opendal.Operator)


def test_async_from_uri_accepts_pure_scheme():
    operator = opendal.AsyncOperator.from_uri("memory")
    assert isinstance(operator, opendal.AsyncOperator)


def test_from_uri_operator_pickle(tmp_path):
    operator = opendal.Operator.from_uri(tmp_path.as_uri())
    operator.write("test", b"hello")

    deserialized = pickle.loads(pickle.dumps(operator))
    assert deserialized.read("test") == b"hello"


def test_from_uri_unsupported_scheme_raises():
    with pytest.raises(Unsupported):
        opendal.Operator.from_uri("thisdoesnotexist")
