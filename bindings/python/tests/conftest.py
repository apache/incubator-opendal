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
from dotenv import load_dotenv

import opendal

load_dotenv()
pytest_plugins = ("pytest_asyncio",)
CAPABILITY_OVERRIDES_ENV = "OPENDAL_TEST_CAPABILITY_OVERRIDES"


def pytest_configure(config):
    # register an additional marker
    config.addinivalue_line(
        "markers",
        "need_capability(*capability): mark test to run only on named capability",
    )


@pytest.fixture(scope="session")
def service_name():
    service_name = os.environ.get("OPENDAL_TEST")
    if service_name is None:
        pytest.skip("OPENDAL_TEST not set")
    return service_name


@pytest.fixture(scope="session")
def setup_config(service_name):
    # Read arguments from envs.
    prefix = f"opendal_{service_name}_"
    config = {}
    for key in os.environ:
        if key.lower().startswith(prefix):
            config[key[len(prefix) :].lower()] = os.environ.get(key)
    disable_random_root = os.environ.get("OPENDAL_DISABLE_RANDOM_ROOT") == "true"
    if not disable_random_root:
        config["root"] = f"{config.get('root', '/')}/{str(uuid4())}/"
    return config


@pytest.fixture(scope="session")
def async_operator(service_name, setup_config):
    operator = (
        opendal.AsyncOperator(service_name, **setup_config)
        .layer(opendal.layers.RetryLayer())
        .layer(opendal.layers.ConcurrentLimitLayer(1024))
        .layer(opendal.layers.MimeGuessLayer())
    )
    overrides = os.environ.get(CAPABILITY_OVERRIDES_ENV)
    if overrides:
        operator = operator.layer(opendal.layers.CapabilityOverrideLayer(overrides))
    return operator


@pytest.fixture(scope="session")
def operator(async_operator):
    return async_operator.to_operator()


@pytest.fixture(autouse=True)
def check_capability(request, operator, async_operator) -> None:
    marker = request.node.get_closest_marker("need_capability")
    if (
        marker
        and marker.args
        and not all(
            [getattr(operator.capability(), x) for x in marker.args]
            + [getattr(async_operator.capability(), x) for x in marker.args]
        )
    ):
        pytest.skip(f"skip because {marker.args} not supported")
