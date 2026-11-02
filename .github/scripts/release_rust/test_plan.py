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

import tempfile
import textwrap
import unittest
from pathlib import Path

from plan import plan


def write_manifest(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(textwrap.dedent(content).strip() + "\n", encoding="utf-8")


class ReleaseRustPlanTest(unittest.TestCase):
    def test_fixture_plan_orders_path_dependencies(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)

            write_manifest(
                root / "core" / "core" / "Cargo.toml",
                """
                [package]
                name = "opendal-core"
                version = "0.1.0"
                """,
            )
            write_manifest(
                root / "core" / "layers" / "retry" / "Cargo.toml",
                """
                [package]
                name = "opendal-layer-retry"
                version = "0.1.0"

                [dependencies]
                opendal-core = { path = "../../core", version = "0.1.0" }
                """,
            )
            write_manifest(
                root / "core" / "services" / "fs" / "Cargo.toml",
                """
                [package]
                name = "opendal-service-fs"
                version = "0.1.0"

                [dependencies]
                opendal-core = { path = "../../core", version = "0.1.0" }
                """,
            )
            write_manifest(
                root / "core" / "Cargo.toml",
                """
                [package]
                name = "opendal"
                version = "0.1.0"

                [dependencies]
                opendal-core = { path = "core", version = "0.1.0" }
                opendal-layer-retry = { path = "layers/retry", version = "0.1.0" }
                opendal-testkit = { path = "testkit", version = "0.1.0", optional = true }

                [target.'cfg(unix)'.build-dependencies]
                opendal-service-fs = { path = "services/fs", version = "0.1.0" }
                """,
            )
            write_manifest(
                root / "core" / "testkit" / "Cargo.toml",
                """
                [package]
                name = "opendal-testkit"
                version = "0.1.0"

                [dependencies]
                opendal-core = { path = "../core", version = "0.1.0" }
                """,
            )
            write_manifest(
                root / "integrations" / "object_store" / "Cargo.toml",
                """
                [package]
                name = "object_store_opendal"
                version = "0.1.0"

                [dependencies]
                opendal = { path = "../../core", version = "0.1.0" }
                """,
            )
            write_manifest(
                root / "core" / "services" / "private" / "Cargo.toml",
                """
                [package]
                name = "opendal-service-private"
                version = "0.1.0"
                publish = false
                """,
            )

            result = plan(root)
            self.assertEqual(
                result,
                [
                    "core/core",
                    "core/layers/retry",
                    "core/services/fs",
                    "core/testkit",
                    "core",
                    "integrations/object_store",
                ],
            )

    def test_repository_plan_excludes_non_release_paths(self):
        result = plan()

        self.assertIn("core/core", result)
        self.assertIn("core/testkit", result)
        self.assertIn("core", result)
        self.assertIn("integrations/object_store", result)

        self.assertNotIn("bindings/python", result)
        self.assertNotIn("core/examples/basic", result)

    def test_repository_plan_orders_core_before_root_and_integrations(self):
        result = plan()

        self.assertLess(result.index("core/core"), result.index("core"))
        self.assertLess(result.index("core/testkit"), result.index("core"))
        self.assertLess(result.index("core"), result.index("integrations/object_store"))
        self.assertLess(result.index("core"), result.index("integrations/parquet"))
        self.assertLess(result.index("core"), result.index("integrations/unftp-sbe"))


if __name__ == "__main__":
    unittest.main()
