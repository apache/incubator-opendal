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

"""Sign a verified source bundle for ATR compose."""

import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
KEYS = "https://downloads.apache.org/opendal/KEYS"
RC = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)-rc\.([1-9][0-9]*)")


def run(*args, data=None, env=None):
    result = subprocess.run(
        [str(arg) for arg in args],
        input=data,
        capture_output=True,
        env=env,
        check=False,
    )
    if result.returncode:
        # Command arguments and diagnostics can contain credentials. Keep them local.
        raise RuntimeError(f"{args[0]} failed with exit status {result.returncode}")
    return result.stdout


def download(url):
    with urllib.request.urlopen(url, timeout=120) as response:
        return response.read()


def sha512(data):
    return hashlib.sha512(data).hexdigest()


def validate(bundle, candidate, rc):
    if not re.fullmatch(r"[0-9a-f]{40}", candidate) or not RC.fullmatch(rc):
        raise ValueError("use a full commit SHA and X.Y.Z-rc.N candidate version")
    report = json.loads((bundle / "report.json").read_text())
    if (
        report["commit"] != candidate
        or report["patch_sha512"] != sha512(b"")
        or not report["matched"]
    ):
        raise ValueError("bundle is not a clean reproduction of the requested commit")
    if len(report["runs"]) != 2 or report["runs"][0] != report["runs"][1]:
        raise ValueError("source reproduction failed")
    files = report["runs"][0]
    # The workflow checkout owns the package allowlist, not the build artifact.
    packages = set(
        re.findall(
            r'make_package\(\s*"([^"\n]+)"',
            (ROOT / "dev/src/release/package.rs").read_text(),
        )
    )
    remaining = {path.replace("/", "-") for path in packages}
    for name, expected in files.items():
        match = re.fullmatch(
            r"apache-opendal-(.+)-([0-9]+\.[0-9]+\.[0-9]+)-src\.tar\.gz", name
        )
        if not match or match[1] not in remaining:
            raise ValueError("unexpected or duplicate source package")
        remaining.remove(match[1])
        if match[1] == "core" and match[2] != rc.split("-rc.")[0]:
            raise ValueError("RC version differs from the core source package")
        path = bundle / name
        if (
            path.is_symlink()
            or not path.is_file()
            or sha512(path.read_bytes()) != expected
        ):
            raise ValueError(f"source checksum mismatch: {name}")
        sidecar = bundle / (name + ".sha512")
        if sidecar.is_symlink() or sidecar.read_text().strip() != f"{expected}  {name}":
            raise ValueError(f"invalid checksum file: {name}")
    if not files or remaining:
        raise ValueError("incomplete source package inventory")
    return files


def verify_signature(home, archive, signature, fingerprint):
    status = run(
        "gpg",
        "--homedir",
        home,
        "--batch",
        "--status-fd",
        "1",
        "--verify",
        signature,
        archive,
    )
    valid = [
        line.split()
        for line in status.decode().splitlines()
        if line.startswith("[GNUPG:] VALIDSIG ")
    ]
    if len(valid) != 1 or valid[0][-1] != fingerprint:
        raise ValueError("signature is not from the configured primary key")


def sign(bundle, candidate, rc, output):
    files = validate(bundle, candidate, rc)
    fingerprint = os.environ["SOURCE_SIGNING_FINGERPRINT"]
    if not re.fullmatch(r"[0-9A-F]{40}", fingerprint):
        raise ValueError("configure the full primary signing fingerprint")
    output.mkdir(parents=True, exist_ok=False)
    # GPG agent socket paths must fit the Unix socket limit, including on macOS.
    with tempfile.TemporaryDirectory(prefix="od-sign-", dir="/tmp") as tmp:
        root = Path(tmp)
        home = root / "gnupg"
        home.mkdir(mode=0o700)
        run("gpg", "--homedir", home, "--batch", "--import", data=download(KEYS))
        run("gpg", "--homedir", home, "--batch", "--list-keys", fingerprint)
        directory = output
        for name in files:
            for suffix in ("", ".sha512"):
                (directory / (name + suffix)).write_bytes(
                    (bundle / (name + suffix)).read_bytes()
                )
        run(
            "gpg",
            "--homedir",
            home,
            "--batch",
            "--import",
            data=os.environ["SOURCE_SIGNING_KEY"].encode(),
        )
        for name in files:
            archive = directory / name
            run(
                "gpg",
                "--homedir",
                home,
                "--batch",
                "--pinentry-mode",
                "loopback",
                "--passphrase-fd",
                "0",
                "--local-user",
                fingerprint,
                "--armor",
                "--detach-sign",
                archive,
                data=(os.environ.get("SOURCE_SIGNING_PASSPHRASE", "") + "\n").encode(),
            )
            verify_signature(home, archive, directory / (name + ".asc"), fingerprint)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("candidate")
    parser.add_argument("rc")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if os.environ.get("GITHUB_EVENT_NAME") not in {"workflow_dispatch", "schedule"}:
        raise ValueError("source signing requires manual dispatch or a scheduled run")
    if not re.fullmatch(r"[0-9a-f]{40}", args.candidate):
        raise ValueError("full candidate SHA required")
    # Recheck reachability in the trusted checkout before obtaining the private key.
    run("git", "merge-base", "--is-ancestor", args.candidate, "HEAD")
    sign(args.bundle, args.candidate, args.rc, args.output)
    print("Verified signatures for the complete source inventory")


if __name__ == "__main__":
    main()
