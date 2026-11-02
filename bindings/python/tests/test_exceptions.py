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

import builtins
import inspect

import pytest

from opendal import exceptions


def test_exceptions():
    for _name, obj in inspect.getmembers(exceptions):
        if inspect.isclass(obj):
            assert issubclass(obj, builtins.Exception)


def test_all_expected_exceptions_present():
    """Verify every expected exception class is present in opendal.exceptions."""
    expected = [
        "Error",
        "Unexpected",
        "Unsupported",
        "ConfigInvalid",
        "NotFound",
        "PermissionDenied",
        "IsADirectory",
        "NotADirectory",
        "AlreadyExists",
        "IsSameFile",
        "ConditionNotMatch",
        "RateLimited",
        "RangeNotSatisfied",
        "Conflict",
    ]
    for name in expected:
        assert hasattr(exceptions, name), f"exceptions.{name} is missing"
        cls = getattr(exceptions, name)
        assert inspect.isclass(cls), f"exceptions.{name} is not a class"
        assert issubclass(cls, builtins.Exception), (
            f"exceptions.{name} does not inherit from Exception"
        )


def test_rate_limited_is_catchable():
    """RateLimited can be raised and caught like a standard exception."""
    msg = "too many requests"
    with pytest.raises(exceptions.RateLimited):
        raise exceptions.RateLimited(msg)


def test_range_not_satisfied_is_catchable():
    """RangeNotSatisfied can be raised and caught like a standard exception."""
    msg = "requested range not satisfiable"
    with pytest.raises(exceptions.RangeNotSatisfied):
        raise exceptions.RangeNotSatisfied(msg)


def test_exceptions_are_distinct():
    """RateLimited and RangeNotSatisfied must not accidentally catch each other."""
    assert not issubclass(exceptions.RateLimited, exceptions.RangeNotSatisfied)
    assert not issubclass(exceptions.RangeNotSatisfied, exceptions.RateLimited)
    assert not issubclass(exceptions.Conflict, exceptions.ConditionNotMatch)
    assert not issubclass(exceptions.ConditionNotMatch, exceptions.Conflict)
