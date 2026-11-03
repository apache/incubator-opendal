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
from pathlib import Path
from unittest.mock import patch

from plan import LANGUAGE_BINDING, group_cases_by_service, plan


class BehaviorTestPlan(unittest.TestCase):
    def test_empty(self):
        result = plan([])
        self.assertEqual(result["components"]["core"], False)
        self.assertEqual(result["components"]["binding_java"], False)
        self.assertEqual(len(result["core"]), 0)
        self.assertEqual(len(result["binding_java"]), 0)

    def test_core_cargo_toml(self):
        result = plan(["core/Cargo.toml"])
        self.assertTrue(result["components"]["core"])

    def test_core_services_fs(self):
        result = plan(["core/services/fs/src/lib.rs"])
        self.assertTrue(result["components"]["core"])
        self.assertTrue(len(result["core"]) > 0)

        cases = [v["service"] for v in result["core"][0]["cases"]]
        # Should contain fs
        self.assertTrue("fs" in cases)
        # Should not contain s3
        self.assertFalse("s3" in cases)

    def test_github_fixture_is_core_only(self):
        result = plan([
            ".github/services/github/read_only/action.yml",
            "core/services/fs/src/lib.rs",
        ])
        core = [case["service"] for target in result["core"] for case in target["cases"]]
        self.assertIn("github", core)
        for language in LANGUAGE_BINDING:
            with self.subTest(language=language):
                cases = [
                    case["service"]
                    for target in result[f"binding_{language}"]
                    for case in target["cases"]
                ]
                self.assertIn("fs", cases)
                self.assertNotIn("github", cases)

    def test_smb_fixture_is_core_only(self):
        result = plan([
            ".github/services/smb/samba/action.yml",
            "core/services/fs/src/lib.rs",
        ])
        core = [case["service"] for target in result["core"] for case in target["cases"]]
        self.assertIn("smb", core)
        for language in LANGUAGE_BINDING:
            with self.subTest(language=language):
                cases = [
                    case["service"]
                    for target in result[f"binding_{language}"]
                    for case in target["cases"]
                ]
                self.assertIn("fs", cases)
                self.assertNotIn("smb", cases)

    def test_core_services_hdfs_native_mapping(self):
        result = plan(["core/services/hdfs-native/src/lib.rs"])
        self.assertTrue(result["components"]["core"])
        self.assertTrue(len(result["core"]) > 0)

        cases = [v["service"] for v in result["core"][0]["cases"]]
        self.assertTrue("hdfs_native" in cases)
        self.assertFalse("fs" in cases)

    def test_group_core_cases_by_service(self):
        cases = [
            {
                "service": "webdav",
                "setup": "nginx_with_empty_password",
                "feature": "services-webdav",
            },
            {
                "service": "webdav",
                "setup": "nginx_with_password",
                "feature": "services-webdav",
            },
            {
                "service": "memory",
                "setup": "memory",
                "feature": "services-memory",
            },
        ]

        self.assertEqual(
            group_cases_by_service(cases),
            [
                {
                    "service": "webdav",
                    "feature": "services-webdav",
                    "setups": [
                        "nginx_with_empty_password",
                        "nginx_with_password",
                    ],
                },
                {
                    "service": "memory",
                    "feature": "services-memory",
                    "setups": ["memory"],
                },
            ],
        )

    def test_core_groups_multiple_setups(self):
        result = plan(["core/services/webdav/src/lib.rs"])
        cases = result["core"][0]["cases"]
        webdav = next(v for v in cases if v["service"] == "webdav")

        self.assertIn("nginx_with_empty_password", webdav["setups"])
        self.assertIn("nginx_with_password", webdav["setups"])
        self.assertIn("nginx_with_redirect", webdav["setups"])
        self.assertEqual(
            len([v for v in cases if v["service"] == "webdav"]),
            1,
        )

    def test_core_action(self):
        result = plan([".github/actions/test_behavior_core/action.yaml"])
        self.assertTrue(result["components"]["core"])
        self.assertTrue(len(result["core"]) > 0)

        for target in result["core"]:
            for case in target["cases"]:
                self.assertIn("setups", case)
                self.assertNotIn("setup", case)

        windows = next(v for v in result["core"] if v["os"] == "windows-latest")
        self.assertEqual(windows["cases"][0]["setups"], ["local_fs"])

    def test_service_setups_do_not_checkout(self):
        service_dir = Path(__file__).parents[2] / "services"
        setup_actions = service_dir.glob("*/*/action.yml")

        for action in setup_actions:
            self.assertNotIn("actions/checkout@", action.read_text(), str(action))

    def test_binding_java(self):
        result = plan(["bindings/java/pom.xml"])
        self.assertFalse(result["components"]["core"])
        self.assertTrue(len(result["core"]) == 0)
        self.assertTrue(result["components"]["binding_java"])
        self.assertTrue(len(result["binding_java"]) > 0)

    def test_binding_java_excludes_hf(self):
        result = plan(["bindings/java/pom.xml"])
        cases = [v["service"] for target in result["binding_java"] for v in target["cases"]]
        self.assertFalse("hf" in cases)

    def test_integration_object_store(self):
        result = plan(["integrations/object_store/Cargo.toml"])
        self.assertTrue(result["components"]["integration_object_store"])
        self.assertTrue(len(result["integration_object_store"]) > 0)

        result = plan(["core/services/fs/src/lib.rs"])
        cases = [v["service"] for v in result["integration_object_store"][0]["cases"]]
        # Should contain fs
        self.assertTrue("fs" in cases)

if __name__ == "__main__":
    unittest.main()
