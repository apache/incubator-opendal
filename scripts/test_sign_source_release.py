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

"""Exercise source inventory validation and real OpenPGP signing."""

import json
import os
import re
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import sign_source_release as signing


class SourceSigningTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bundle = self.root / "bundle"
        self.bundle.mkdir()
        self.candidate = "a" * 40
        self.rc = "0.59.2-rc.1"
        self.files = {}
        packages = re.findall(
            r'make_package\(\s*"([^"\n]+)"',
            (signing.ROOT / "dev/src/release/package.rs").read_text(),
        )
        for package in packages:
            name = f"apache-opendal-{package.replace('/', '-')}-0.59.2-src.tar.gz"
            data = package.encode()
            digest = signing.sha512(data)
            (self.bundle / name).write_bytes(data)
            (self.bundle / (name + ".sha512")).write_text(f"{digest}  {name}\n")
            self.files[name] = digest
        self.report = {
            "commit": self.candidate,
            "patch_sha512": signing.sha512(b""),
            "matched": True,
            "runs": [self.files.copy(), self.files.copy()],
        }
        self.write_report()

    def write_report(self):
        (self.bundle / "report.json").write_text(json.dumps(self.report))

    def test_signing_entrypoint_events_and_reachability(self):
        repository = self.root / "repository"
        repository.mkdir()
        previous = Path.cwd()
        try:
            os.chdir(repository)
            signing.run("git", "init", "-b", "main")
            signing.run(
                "git",
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.org",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "Initial",
            )
            signing.run("git", "checkout", "-b", "release/0.59.2")
            signing.run(
                "git",
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.org",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "Bump version",
            )
            candidate = signing.run("git", "rev-parse", "HEAD").decode().strip()
            argv = [
                "sign_source_release.py",
                str(self.bundle),
                candidate,
                self.rc,
                str(self.root / "signed"),
            ]
            with (
                patch.dict(
                    os.environ,
                    {
                        "GITHUB_REF": "refs/heads/release/0.59.2",
                        "GITHUB_EVENT_NAME": "workflow_dispatch",
                    },
                ),
                patch("sys.argv", argv),
                patch.object(signing, "sign") as sign,
            ):
                for event in ("workflow_dispatch", "schedule"):
                    with self.subTest(event=event), patch.dict(
                        os.environ, {"GITHUB_EVENT_NAME": event}
                    ):
                        sign.reset_mock()
                        signing.main()
                        sign.assert_called_once_with(
                            self.bundle, candidate, self.rc, self.root / "signed"
                        )
                for event in ("push", "pull_request", "pull_request_target", ""):
                    with self.subTest(event=event), patch.dict(
                        os.environ, {"GITHUB_EVENT_NAME": event}
                    ):
                        sign.reset_mock()
                        with self.assertRaisesRegex(ValueError, "scheduled run"):
                            signing.main()
                        sign.assert_not_called()
                # The same candidate is not reachable when dispatching from main.
                signing.run("git", "checkout", "main")
                sign.reset_mock()
                with self.assertRaises(RuntimeError):
                    signing.main()
                sign.assert_not_called()
        finally:
            os.chdir(previous)

    def test_complete_inventory(self):
        self.assertEqual(
            signing.validate(self.bundle, self.candidate, self.rc), self.files
        )

    def test_tampered_archive(self):
        (self.bundle / next(iter(self.files))).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            signing.validate(self.bundle, self.candidate, self.rc)

    def test_incomplete_inventory(self):
        name = next(iter(self.files))
        for run in self.report["runs"]:
            del run[name]
        self.write_report()
        with self.assertRaisesRegex(ValueError, "incomplete"):
            signing.validate(self.bundle, self.candidate, self.rc)

    def test_identity_and_reproduction(self):
        for key, value in [
            ("commit", "b" * 40),
            ("matched", False),
            ("patch_sha512", "patch"),
        ]:
            with self.subTest(key=key):
                original = self.report[key]
                self.report[key] = value
                self.write_report()
                with self.assertRaises(ValueError):
                    signing.validate(self.bundle, self.candidate, self.rc)
                self.report[key] = original
        self.write_report()
        with self.assertRaisesRegex(ValueError, "RC version"):
            signing.validate(self.bundle, self.candidate, "0.60.0-rc.1")
        with self.assertRaises(ValueError):
            signing.validate(self.bundle, self.candidate, "../../other")

    def test_symlink_rejected(self):
        archive = self.bundle / next(iter(self.files))
        target = self.root / "target"
        archive.rename(target)
        archive.symlink_to(target)
        with self.assertRaises(ValueError):
            signing.validate(self.bundle, self.candidate, self.rc)

    def test_sign_and_verify_complete_bundle(self):
        home = self.root / "keys"
        home.mkdir(mode=0o700)
        gpg = [
            "gpg",
            "--homedir",
            home,
            "--batch",
            "--pinentry-mode",
            "loopback",
            "--passphrase",
            "",
        ]
        signing.run(
            *gpg,
            "--quick-generate-key",
            "Test <test@example.invalid>",
            "ed25519",
            "sign",
            "0",
        )
        listing = signing.run(*gpg, "--with-colons", "--list-keys").decode()
        fingerprint = next(
            line.split(":")[9]
            for line in listing.splitlines()
            if line.startswith("fpr:")
        )
        public = signing.run(*gpg, "--armor", "--export", fingerprint)
        private = signing.run(*gpg, "--armor", "--export-secret-keys", fingerprint)
        output = self.root / "signed"
        with (
            patch.dict(
                os.environ,
                {
                    "SOURCE_SIGNING_FINGERPRINT": fingerprint,
                    "SOURCE_SIGNING_KEY": private.decode(),
                    "SOURCE_SIGNING_PASSPHRASE": "",
                },
            ),
            patch.object(signing, "download", return_value=public),
        ):
            signing.sign(self.bundle, self.candidate, self.rc, output)
        expected = {
            name + suffix for name in self.files for suffix in ("", ".sha512", ".asc")
        }
        self.assertEqual({p.name for p in output.iterdir()}, expected)
        name = next(iter(self.files))
        with self.assertRaisesRegex(ValueError, "configured primary key"):
            signing.verify_signature(
                home, output / name, output / (name + ".asc"), "F" * 40
            )
        (output / name).write_bytes(b"tampered")
        with self.assertRaises(RuntimeError):
            signing.verify_signature(
                home, output / name, output / (name + ".asc"), fingerprint
            )


if __name__ == "__main__":
    unittest.main()
