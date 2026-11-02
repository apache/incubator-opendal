#!/bin/bash
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

set -e

echo "=== Testing OpenDAL C Binding Test Framework CI Integration ==="
echo

# Test 1: Framework can be built
echo "1. Building test framework..."
cd "$(dirname "$0")"
make clean > /dev/null 2>&1 || true
make > /dev/null 2>&1
echo "✓ Test framework builds successfully"

# Test 2: Framework can run tests
echo "2. Testing with memory service..."
OPENDAL_TEST=memory timeout 30 make behavior_test > /dev/null 2>&1
echo "✓ Memory service tests run successfully"

# Test 3: Framework handles unsupported services gracefully
echo "3. Testing with unsupported service..."
if OPENDAL_TEST=invalid_service timeout 10 make behavior_test > /dev/null 2>&1; then
    echo "✗ Should have failed with invalid service"
    exit 1
else
    echo "✓ Gracefully handles unsupported services"
fi

# Test 4: Verification script works
echo "4. Running verification script..."
./verify_framework.sh > /dev/null 2>&1
echo "✓ Verification script runs successfully"

echo
echo "=== All CI Integration Tests Passed! ==="
echo "The C binding test framework is ready for CI/CD integration." 