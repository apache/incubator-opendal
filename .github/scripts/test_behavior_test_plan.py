#!/usr/bin/env python3
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

import unittest
from behavior_test_plan import plan


class BehaviorTestPlan(unittest.TestCase):
    def test_empty(self):
        result = plan([])
        self.assertEqual(result, {})

    def test_core_cargo_toml(self):
        result = plan(["core/Cargo.toml"])
        self.assertTrue(result["components"]["core"])

    def test_core_services_fs(self):
        result = plan(["core/src/services/fs/mod.rs"])
        self.assertTrue(result["components"]["core"])
        self.assertTrue(len(result["core"]) > 0)
        # Should not contain fs
        self.assertTrue("services-fs" in result["core"][0]["features"])
        # Should not contain s3
        self.assertFalse("services-s3" in result["core"][0]["features"])


if __name__ == "__main__":
    unittest.main()
