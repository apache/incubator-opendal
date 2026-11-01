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
from contextlib import contextmanager
from pathlib import Path
from unittest import mock

from publish import local_dev_dependency_names
from publish import load_manifest
from publish import publish_package
from publish import strip_local_dev_dependencies


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(textwrap.dedent(content).strip() + "\n", encoding="utf-8")


class ReleaseRustPublishTest(unittest.TestCase):
    def test_strip_local_dev_dependencies(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            package = root / "crate"
            write(
                package / "Cargo.toml",
                """
                [package]
                name = "crate"
                version = "0.1.0"

                [dependencies]
                local-normal = { path = "../local-normal", version = "0.1.0" }

                [dev-dependencies]
                external-dev = "1"
                local-dev = { path = "../local-dev", version = "0.1.0" }
                local-dev-multiline = { path = "../local-dev-multiline", version = "0.1.0", features = [
                  "test",
                ] }

                [target.'cfg(unix)'.dev-dependencies]
                local-target-dev = { path = "../local-target-dev", version = "0.1.0" }
                """,
            )
            for name in (
                "local-normal",
                "local-dev",
                "local-dev-multiline",
                "local-target-dev",
            ):
                (root / name).mkdir()

            manifest = load_manifest(package / "Cargo.toml")
            names = local_dev_dependency_names(manifest, package)
            self.assertEqual(
                names,
                {"local-dev", "local-dev-multiline", "local-target-dev"},
            )

            changed = strip_local_dev_dependencies(package / "Cargo.toml", names)
            self.assertTrue(changed)

            stripped = (package / "Cargo.toml").read_text()
            self.assertIn("local-normal", stripped)
            self.assertIn("external-dev", stripped)
            self.assertNotIn("local-dev =", stripped)
            self.assertNotIn("local-dev-multiline", stripped)
            self.assertNotIn("local-target-dev", stripped)

    def test_live_publish_fetches_a_new_token_for_every_attempt(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            package = root / "crate"
            write(
                package / "Cargo.toml",
                """
                [package]
                name = "crate"
                version = "0.1.0"
                """,
            )

            tokens = iter(("first-token", "second-token"))
            released: list[str] = []
            cargo_tokens: list[str | None] = []

            @contextmanager
            def token_provider():
                token = next(tokens)
                try:
                    yield token
                finally:
                    released.append(token)

            results = iter(
                (
                    subprocess_result(
                        1, "Too many requests. Please try again after invalid."
                    ),
                    subprocess_result(0, "published"),
                )
            )

            def run(*args, **kwargs):
                cargo_tokens.append(kwargs["env"].get("CARGO_REGISTRY_TOKEN"))
                return next(results)

            with (
                mock.patch.dict(
                    "publish.os.environ",
                    {"CARGO_REGISTRY_TOKEN": "legacy-token"},
                    clear=True,
                ),
                mock.patch(
                    "publish.temporary_trusted_publishing_token", token_provider
                ),
                mock.patch("publish.subprocess.run", run),
                mock.patch("publish.time.sleep") as sleep,
            ):
                publish_package(
                    root,
                    "crate",
                    dry_run=False,
                )

            self.assertEqual(cargo_tokens, ["first-token", "second-token"])
            self.assertEqual(released, ["first-token", "second-token"])
            sleep.assert_called_once_with(610)

    def test_dry_run_does_not_request_a_trusted_publishing_token(self):
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            package = root / "crate"
            write(
                package / "Cargo.toml",
                """
                [package]
                name = "crate"
                version = "0.1.0"
                """,
            )

            with (
                mock.patch.dict(
                    "publish.os.environ",
                    {"CARGO_REGISTRY_TOKEN": "legacy-token"},
                    clear=True,
                ),
                mock.patch(
                    "publish.temporary_trusted_publishing_token"
                ) as token_provider,
                mock.patch(
                    "publish.subprocess.run",
                    return_value=subprocess_result(0, "checked"),
                ) as run,
            ):
                publish_package(root, "crate", dry_run=True)

            token_provider.assert_not_called()
            self.assertIn("--dry-run", run.call_args.args[0])
            self.assertNotIn("CARGO_REGISTRY_TOKEN", run.call_args.kwargs["env"])


def subprocess_result(returncode: int, output: str):
    return mock.Mock(returncode=returncode, stdout=output)


if __name__ == "__main__":
    unittest.main()
