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

import argparse
import io
import subprocess
import tempfile
import tomllib
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

from bootstrap import CratesIoClient
from bootstrap import PLACEHOLDER_DESCRIPTION
from bootstrap import PLACEHOLDER_VERSION
from bootstrap import PUBLISHER
from bootstrap import PROJECT_DIR
from bootstrap import REPOSITORY
from bootstrap import PlannedCrate
from bootstrap import ReconcileResult
from bootstrap import _placeholder_manifest
from bootstrap import discover
from bootstrap import preflight_authenticated
from bootstrap import reconcile_crate
from bootstrap import run_apply
from bootstrap import verify_authenticated
from bootstrap import write_placeholder_package


def metadata(name: str, *, version: str = "1.0.0", trustpub_only: bool = False):
    return {
        "id": name,
        "max_version": version,
        "repository": REPOSITORY,
        "description": (
            PLACEHOLDER_DESCRIPTION
            if version == PLACEHOLDER_VERSION
            else "An Apache OpenDAL crate"
        ),
        "trustpub_only": trustpub_only,
    }


def expected_config(name: str):
    return {"crate": name, **PUBLISHER}


class FakeClient:
    def __init__(self, name: str, krate=None, configs=None):
        self.name = name
        self.krate = krate
        self.configs = list(configs or [])
        self.created_configs = 0
        self.restricted = 0

    def get_crate(self, name: str):
        self._assert_name(name)
        return None if self.krate is None else dict(self.krate)

    def get_version(self, name: str, version: str):
        self._assert_name(name)
        return {"num": version}

    def list_github_configs(self, name: str):
        self._assert_name(name)
        return [dict(config) for config in self.configs]

    def create_github_config(self, name: str):
        self._assert_name(name)
        config = expected_config(name)
        self.configs.append(config)
        self.created_configs += 1
        return dict(config)

    def set_trustpub_only(self, name: str):
        self._assert_name(name)
        self.krate["trustpub_only"] = True
        self.restricted += 1
        return dict(self.krate)

    def _assert_name(self, name: str):
        if name != self.name:
            raise AssertionError(f"expected {self.name}, got {name}")


class JsonResponse(io.BytesIO):
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.close()


class BootstrapTest(unittest.TestCase):
    def test_bootstrap_workflow_is_input_free_and_always_protected(self):
        workflow = (
            PROJECT_DIR / ".github/workflows/bootstrap_rust_crates.yml"
        ).read_text(encoding="utf-8")
        dispatch = workflow.split("\non:\n", 1)[1].split("\npermissions:\n", 1)[0]

        self.assertEqual(dispatch, "  workflow_dispatch:\n")
        self.assertIn("    environment: rust-bootstrap\n", workflow)
        self.assertNotIn("candidate_count", workflow)

    def test_publisher_matches_the_release_workflow(self):
        workflow = (PROJECT_DIR / ".github/workflows/release_rust.yml").read_text(
            encoding="utf-8"
        )
        publish_job = workflow.split("\n  publish:\n", 1)[1]

        self.assertEqual(PUBLISHER["workflow_filename"], "release_rust.yml")
        self.assertEqual(PUBLISHER["environment"], "release")
        self.assertIn("    environment: release\n", publish_job)
        self.assertIn("      id-token: write\n", publish_job)

    def test_placeholder_manifest_is_dependency_free(self):
        manifest = tomllib.loads(_placeholder_manifest("opendal-service-new"))

        self.assertEqual(manifest["package"]["name"], "opendal-service-new")
        self.assertEqual(manifest["package"]["version"], PLACEHOLDER_VERSION)
        self.assertNotIn("dependencies", manifest)
        self.assertNotIn("dev-dependencies", manifest)
        self.assertNotIn("build-dependencies", manifest)

    def test_placeholder_can_be_packaged_offline(self):
        planned = PlannedCrate("opendal-service-new", "core/services/new")
        with tempfile.TemporaryDirectory() as tmpdir:
            package_dir = Path(tmpdir)
            write_placeholder_package(PROJECT_DIR, planned, package_dir)

            process = subprocess.run(
                [
                    "cargo",
                    "package",
                    "--manifest-path",
                    str(package_dir / "Cargo.toml"),
                    "--no-verify",
                    "--offline",
                ],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
            )

            self.assertEqual(process.returncode, 0, process.stdout)
            self.assertTrue(
                (
                    package_dir
                    / "target"
                    / "package"
                    / f"{planned.name}-{PLACEHOLDER_VERSION}.crate"
                ).is_file()
            )

    def test_public_read_retries_a_transient_registry_failure(self):
        error = urllib.error.HTTPError(
            "https://crates.io/api/v1/crates/opendal",
            503,
            "unavailable",
            {},
            io.BytesIO(b""),
        )
        response = JsonResponse(
            b'{"crate":{"id":"opendal","repository":'
            b'"https://github.com/apache/opendal"}}'
        )

        with (
            mock.patch(
                "bootstrap.urllib.request.urlopen",
                side_effect=(error, response),
            ),
            mock.patch("bootstrap.time.sleep") as sleep,
        ):
            krate = CratesIoClient().get_crate("opendal")

        self.assertEqual(krate["id"], "opendal")
        sleep.assert_called_once_with(1)

    def test_discovery_does_not_migrate_established_crates(self):
        planned = PlannedCrate("opendal-core", "core/core")
        client = FakeClient(planned.name, metadata(planned.name))

        with mock.patch("bootstrap.planned_crates", return_value=[planned]):
            packages, missing, incomplete = discover(Path(), client)

        self.assertEqual(packages, [planned])
        self.assertEqual(missing, [])
        self.assertEqual(incomplete, [])

    def test_discovery_selects_an_incomplete_placeholder(self):
        planned = PlannedCrate("opendal-service-new", "core/services/new")
        client = FakeClient(
            planned.name,
            metadata(planned.name, version=PLACEHOLDER_VERSION),
        )

        with mock.patch("bootstrap.planned_crates", return_value=[planned]):
            _, missing, incomplete = discover(Path(), client)

        self.assertEqual(missing, [])
        self.assertEqual(incomplete, [planned.name])

    def test_discovery_selects_a_ready_placeholder(self):
        planned = PlannedCrate("opendal-service-new", "core/services/new")
        client = FakeClient(
            planned.name,
            metadata(
                planned.name,
                version=PLACEHOLDER_VERSION,
                trustpub_only=True,
            ),
            [expected_config(planned.name)],
        )

        with mock.patch("bootstrap.planned_crates", return_value=[planned]):
            _, missing, placeholders = discover(Path(), client)

        self.assertEqual(missing, [])
        self.assertEqual(placeholders, [planned.name])

    def test_authenticated_preflight_audits_an_established_crate(self):
        planned = PlannedCrate("opendal-core", "core/core")
        client = FakeClient(
            planned.name,
            metadata(planned.name, trustpub_only=True),
            [expected_config(planned.name)],
        )

        verified = preflight_authenticated([planned], set(), client)

        self.assertEqual(verified, [planned.name])
        self.assertEqual(client.created_configs, 0)
        self.assertEqual(client.restricted, 0)

    def test_authenticated_preflight_rejects_an_unexpected_publisher(self):
        planned = PlannedCrate("opendal-core", "core/core")
        unexpected = {
            **expected_config(planned.name),
            "workflow_filename": "other.yml",
        }
        client = FakeClient(
            planned.name,
            metadata(planned.name, trustpub_only=True),
            [unexpected],
        )

        with self.assertRaisesRegex(RuntimeError, "unexpected Trusted Publisher"):
            preflight_authenticated([planned], set(), client)

        self.assertEqual(client.created_configs, 0)
        self.assertEqual(client.restricted, 0)

    def test_authenticated_preflight_rejects_a_ready_placeholder_with_wrong_publisher(
        self,
    ):
        planned = PlannedCrate("opendal-service-new", "core/services/new")
        unexpected = {
            **expected_config(planned.name),
            "workflow_filename": "other.yml",
        }
        client = FakeClient(
            planned.name,
            metadata(
                planned.name,
                version=PLACEHOLDER_VERSION,
                trustpub_only=True,
            ),
            [unexpected],
        )

        with self.assertRaisesRegex(RuntimeError, "unexpected Trusted Publisher"):
            preflight_authenticated([planned], {planned.name}, client)

        self.assertEqual(client.created_configs, 0)
        self.assertEqual(client.restricted, 0)

    def test_authenticated_preflight_rejects_unmigrated_established_crate(self):
        planned = PlannedCrate("opendal-core", "core/core")
        client = FakeClient(
            planned.name,
            metadata(planned.name),
            [expected_config(planned.name)],
        )

        with self.assertRaisesRegex(RuntimeError, "does not require"):
            preflight_authenticated([planned], set(), client)

    def test_authenticated_verification_requires_the_exact_publisher(self):
        planned = PlannedCrate("opendal-core", "core/core")
        unexpected = {
            **expected_config(planned.name),
            "environment": "other",
        }
        client = FakeClient(
            planned.name,
            metadata(planned.name, trustpub_only=True),
            [unexpected],
        )

        with self.assertRaisesRegex(RuntimeError, "unexpected Trusted Publisher"):
            verify_authenticated([planned], client)

    def test_apply_preflights_the_complete_plan_before_reconciling(self):
        candidate = PlannedCrate("opendal-service-new", "core/services/new")
        established = PlannedCrate("opendal-core", "core/core")
        packages = [candidate, established]
        client = object()
        calls = []

        def preflight(actual_packages, candidate_names, actual_client):
            calls.append(("preflight", actual_packages, candidate_names, actual_client))

        def reconcile(project_dir, planned, actual_client, token):
            calls.append(("reconcile", planned, actual_client, token))
            return ReconcileResult(planned.name, ("verified",))

        def verify(actual_packages, actual_client):
            calls.append(("verify", actual_packages, actual_client))
            return [package.name for package in actual_packages]

        args = argparse.Namespace(project_dir=Path(), registry_url="registry")
        with (
            mock.patch.dict(
                "bootstrap.os.environ",
                {"CARGO_REGISTRY_BOOTSTRAP_TOKEN": "bootstrap-token"},
                clear=True,
            ),
            mock.patch("bootstrap.CratesIoClient", return_value=client),
            mock.patch(
                "bootstrap.discover",
                return_value=(packages, [candidate.name], []),
            ),
            mock.patch("bootstrap.preflight_authenticated", preflight),
            mock.patch("bootstrap.reconcile_crate", reconcile),
            mock.patch("bootstrap.verify_authenticated", verify),
        ):
            self.assertEqual(run_apply(args), 0)

        self.assertEqual(
            [call[0] for call in calls],
            ["preflight", "reconcile", "verify"],
        )
        self.assertEqual(calls[0][1], packages)
        self.assertEqual(calls[0][2], {candidate.name})
        self.assertEqual(calls[2][1], packages)

    def test_apply_does_not_mutate_when_authenticated_preflight_fails(self):
        candidate = PlannedCrate("opendal-service-new", "core/services/new")
        args = argparse.Namespace(project_dir=Path(), registry_url="registry")

        with (
            mock.patch.dict(
                "bootstrap.os.environ",
                {"CARGO_REGISTRY_BOOTSTRAP_TOKEN": "bootstrap-token"},
                clear=True,
            ),
            mock.patch("bootstrap.CratesIoClient"),
            mock.patch(
                "bootstrap.discover",
                return_value=([candidate], [candidate.name], []),
            ),
            mock.patch(
                "bootstrap.preflight_authenticated",
                side_effect=RuntimeError("audit failed"),
            ),
            mock.patch("bootstrap.reconcile_crate") as reconcile,
            mock.patch("bootstrap.verify_authenticated") as verify,
            self.assertRaisesRegex(RuntimeError, "audit failed"),
        ):
            run_apply(args)

        reconcile.assert_not_called()
        verify.assert_not_called()

    def test_missing_crate_is_created_configured_and_restricted(self):
        planned = PlannedCrate("opendal-service-new", "core/services/new")
        client = FakeClient(planned.name)

        def publish(project_dir, package, token):
            self.assertEqual(package, planned)
            self.assertEqual(token, "bootstrap-token")
            client.krate = metadata(planned.name, version=PLACEHOLDER_VERSION)

        with (
            tempfile.TemporaryDirectory() as tmpdir,
            mock.patch("bootstrap.publish_placeholder", publish),
        ):
            result = reconcile_crate(Path(tmpdir), planned, client, "bootstrap-token")

        self.assertEqual(
            result.actions,
            (
                "created placeholder",
                "configured Trusted Publishing",
                "enabled Trusted Publishing only",
            ),
        )
        self.assertEqual(client.created_configs, 1)
        self.assertEqual(client.restricted, 1)

    def test_partial_placeholder_resumes_without_republishing(self):
        planned = PlannedCrate("opendal-service-new", "core/services/new")
        client = FakeClient(
            planned.name,
            metadata(planned.name, version=PLACEHOLDER_VERSION),
        )

        with (
            tempfile.TemporaryDirectory() as tmpdir,
            mock.patch("bootstrap.publish_placeholder") as publish,
        ):
            result = reconcile_crate(Path(tmpdir), planned, client, "bootstrap-token")

        publish.assert_not_called()
        self.assertEqual(
            result.actions,
            (
                "configured Trusted Publishing",
                "enabled Trusted Publishing only",
            ),
        )

    def test_ready_placeholder_is_a_noop(self):
        planned = PlannedCrate("opendal-service-new", "core/services/new")
        client = FakeClient(
            planned.name,
            metadata(
                planned.name,
                version=PLACEHOLDER_VERSION,
                trustpub_only=True,
            ),
            [expected_config(planned.name)],
        )

        with tempfile.TemporaryDirectory() as tmpdir:
            result = reconcile_crate(Path(tmpdir), planned, client, "bootstrap-token")

        self.assertEqual(result.actions, ("verified",))
        self.assertEqual(client.created_configs, 0)
        self.assertEqual(client.restricted, 0)

    def test_established_crate_is_never_migrated(self):
        planned = PlannedCrate("opendal-core", "core/core")
        client = FakeClient(planned.name, metadata(planned.name))

        with (
            tempfile.TemporaryDirectory() as tmpdir,
            self.assertRaisesRegex(RuntimeError, "established crate"),
        ):
            reconcile_crate(Path(tmpdir), planned, client, "bootstrap-token")

        self.assertEqual(client.created_configs, 0)
        self.assertEqual(client.restricted, 0)

    def test_unexpected_publisher_fails_closed(self):
        planned = PlannedCrate("opendal-core", "core/core")
        unexpected = {
            **expected_config(planned.name),
            "workflow_filename": "other.yml",
        }
        client = FakeClient(
            planned.name,
            metadata(planned.name, version=PLACEHOLDER_VERSION),
            [unexpected],
        )

        with (
            tempfile.TemporaryDirectory() as tmpdir,
            self.assertRaisesRegex(RuntimeError, "unexpected Trusted Publisher"),
        ):
            reconcile_crate(Path(tmpdir), planned, client, "bootstrap-token")

        self.assertEqual(client.restricted, 0)


if __name__ == "__main__":
    unittest.main()
