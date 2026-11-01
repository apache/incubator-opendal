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
from opendal.exceptions import ConfigInvalid

# Construction and input validation only. Retry behavior is not observable from
# Python yet (no fault injection or attempt hook exposed); it is tested in the
# Rust core and can be added here once a layer like ChaosLayer is bound.


@pytest.mark.parametrize(
    "kwargs",
    [
        {"factor": 0.0},
        {"factor": 0.5},
        {"factor": float("nan")},
        {"factor": float("inf")},
        {"max_delay": -1.0},
        {"max_delay": float("nan")},
        {"max_delay": float("inf")},
        {"max_delay": 1e30},
        {"min_delay": -5.0},
        {"min_delay": float("nan")},
        {"min_delay": float("inf")},
        {"min_delay": 1e30},
    ],
)
def test_retry_layer_rejects_invalid_values(kwargs):
    with pytest.raises(ConfigInvalid):
        opendal.layers.RetryLayer(**kwargs)


@pytest.mark.parametrize(
    "kwargs",
    [
        {},
        {"factor": 1.0},
        {"factor": 2.5},
        {"max_delay": 0.0},
        {"max_delay": 60.0},
        {"min_delay": 0.0},
        {"min_delay": 1.0},
        {"max_times": 5, "factor": 2.0, "max_delay": 30.0, "min_delay": 1.0},
    ],
)
def test_retry_layer_accepts_valid_values(kwargs):
    assert isinstance(opendal.layers.RetryLayer(**kwargs), opendal.layers.RetryLayer)
