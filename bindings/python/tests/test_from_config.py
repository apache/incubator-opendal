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
from pathlib import PurePosixPath

import pytest

import opendal
from opendal.config import FsConfig, S3Config
from opendal.exceptions import Unsupported


@pytest.mark.need_capability("shared")
def test_from_config_constructs_configured_service(service_name, setup_config):
    operator = opendal.Operator.from_config(
        {"scheme": service_name, **setup_config}  # type: ignore[arg-type]
    )
    assert isinstance(operator, opendal.Operator)


def test_async_from_config_returns_expected_type():
    operator = opendal.AsyncOperator.from_config({"scheme": "memory"})
    assert isinstance(operator, opendal.AsyncOperator)


def test_from_config_operator_pickle(tmp_path):
    operator = opendal.Operator.from_config(FsConfig(scheme="fs", root=tmp_path))
    operator.write("test", b"hello")

    deserialized = pickle.loads(pickle.dumps(operator))
    assert deserialized.read("test") == b"hello"


def test_from_config_unsupported_scheme_raises():
    with pytest.raises(Unsupported):
        opendal.Operator.from_config({"scheme": "thisdoesnotexist"})  # type: ignore[arg-type]


def test_from_config_missing_scheme_raises():
    with pytest.raises(Unsupported):
        opendal.Operator.from_config({"bucket": "b"})  # type: ignore[arg-type]


def test_config_is_plain_dict_at_runtime():
    cfg = S3Config(scheme="s3", bucket="b")
    assert cfg == {"scheme": "s3", "bucket": "b"}
    assert type(cfg) is dict


def test_from_config_stringifies_native_values():
    # Non-string values are converted to the flat string map core consumes; the
    # always-enabled memory service ignores the probe keys.
    op = opendal.Operator.from_config(
        {"scheme": "memory", "probe_flag": True, "probe_path": PurePosixPath("/x")}  # type: ignore[arg-type]
    )
    _, args = op.__reduce__()
    assert args[1]["probe_flag"] == "true"
    assert args[1]["probe_path"] == "/x"
