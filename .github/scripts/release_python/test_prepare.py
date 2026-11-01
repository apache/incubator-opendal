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
import tomllib
import unittest
from pathlib import Path

from prepare import (
    PreparationError,
    prepare,
    release_candidate_version,
    render_pyproject,
)


def write_fixture(path: Path, content: str) -> None:
    path.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")


class ReleasePythonPrepareTest(unittest.TestCase):
    def test_prepare_renders_pep_440_release_candidate_version(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            cargo_manifest = root / "Cargo.toml"
            pyproject = root / "pyproject.toml"
            write_fixture(
                cargo_manifest,
                """
                [package]
                name = "opendal-python"
                version = "0.47.5"
                """,
            )
            write_fixture(
                pyproject,
                """
                [project]
                name = "opendal"
                dynamic = ['version']

                [tool.maturin]
                module-name = "opendal._opendal"
                """,
            )
            original_cargo_manifest = cargo_manifest.read_text(encoding="utf-8")

            version = prepare("v0.58.1-rc.5", cargo_manifest, pyproject)

            self.assertEqual(version, "0.47.5rc5")
            self.assertEqual(
                cargo_manifest.read_text(encoding="utf-8"), original_cargo_manifest
            )
            with pyproject.open("rb") as fp:
                project = tomllib.load(fp)["project"]
            self.assertEqual(project["version"], "0.47.5rc5")
            self.assertNotIn("dynamic", project)

    def test_release_candidates_map_to_distinct_versions(self):
        first = release_candidate_version("0.47.5", "v0.58.1-rc.1")
        second = release_candidate_version("0.47.5", "v0.58.1-rc.2")

        self.assertEqual(first, "0.47.5rc1")
        self.assertEqual(second, "0.47.5rc2")
        self.assertNotEqual(first, second)

    def test_prepare_is_idempotent(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            cargo_manifest = root / "Cargo.toml"
            pyproject = root / "pyproject.toml"
            write_fixture(
                cargo_manifest,
                """
                [package]
                version = "0.47.5"
                """,
            )
            write_fixture(
                pyproject,
                """
                [project]
                name = "opendal"
                version = "0.47.5rc5"
                """,
            )
            original = pyproject.read_text(encoding="utf-8")

            version = prepare("v0.58.1-rc.5", cargo_manifest, pyproject)

            self.assertEqual(version, "0.47.5rc5")
            self.assertEqual(pyproject.read_text(encoding="utf-8"), original)

    def test_prepare_rejects_non_release_candidate_tag(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            cargo_manifest = root / "Cargo.toml"
            pyproject = root / "pyproject.toml"
            write_fixture(
                cargo_manifest,
                """
                [package]
                version = "0.47.5"
                """,
            )
            write_fixture(
                pyproject,
                """
                [project]
                dynamic = ['version']
                """,
            )

            with self.assertRaisesRegex(
                PreparationError, "vX.Y.Z-rc.N release candidate format"
            ):
                prepare("v0.58.1-beta.1", cargo_manifest, pyproject)

    def test_prepare_rejects_non_stable_cargo_version(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            cargo_manifest = root / "Cargo.toml"
            pyproject = root / "pyproject.toml"
            write_fixture(
                cargo_manifest,
                """
                [package]
                version = "0.47.5-rc.1"
                """,
            )
            write_fixture(
                pyproject,
                """
                [project]
                dynamic = ['version']
                """,
            )

            with self.assertRaisesRegex(PreparationError, "stable package version"):
                prepare("v0.58.1-rc.1", cargo_manifest, pyproject)

    def test_render_rejects_additional_dynamic_metadata(self):
        content = textwrap.dedent(
            """
            [project]
            dynamic = ["version", "readme"]
            """
        ).lstrip()

        with self.assertRaisesRegex(PreparationError, "only version as dynamic"):
            render_pyproject(content, "0.47.5rc1")

    def test_render_rejects_multiline_dynamic_version(self):
        content = textwrap.dedent(
            """
            [project]
            dynamic = [
              "version",
            ]
            """
        ).lstrip()

        with self.assertRaisesRegex(PreparationError, "must be on one line"):
            render_pyproject(content, "0.47.5rc1")


if __name__ == "__main__":
    unittest.main()
