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

import argparse
import json
import os
import shutil
import subprocess
import tempfile
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path

from plan import plan
from publish import parse_retry_after
from publish import should_retry


SCRIPT_PATH = Path(__file__).resolve()
PROJECT_DIR = SCRIPT_PATH.parents[3]
REGISTRY_URL = "https://crates.io"
REPOSITORY = "https://github.com/apache/opendal"
PLACEHOLDER_VERSION = "0.0.0"
PLACEHOLDER_DESCRIPTION = "Namespace reservation for a crate planned by Apache OpenDAL."
PUBLISHER = {
    "repository_owner": "apache",
    "repository_name": "opendal",
    "workflow_filename": "release_rust.yml",
    "environment": "release",
}
USER_AGENT = "apache-opendal-release-bootstrap/1.0 (https://github.com/apache/opendal)"


class ApiError(RuntimeError):
    def __init__(self, operation: str, status: int, detail: str):
        super().__init__(f"{operation} failed with HTTP {status}: {detail}")
        self.status = status


@dataclass(frozen=True)
class PlannedCrate:
    name: str
    path: str


@dataclass(frozen=True)
class ReconcileResult:
    name: str
    actions: tuple[str, ...]


def planned_crates(project_dir: Path = PROJECT_DIR) -> list[PlannedCrate]:
    project_dir = project_dir.resolve()
    crates: list[PlannedCrate] = []
    names: set[str] = set()

    for package_path in plan(project_dir):
        manifest_path = project_dir / package_path / "Cargo.toml"
        with manifest_path.open("rb") as fp:
            package = tomllib.load(fp)["package"]

        name = package["name"]
        if name in names:
            raise RuntimeError(
                f"duplicate crates.io package name in publish plan: {name}"
            )
        names.add(name)
        crates.append(PlannedCrate(name=name, path=package_path))

    return crates


def _api_error_detail(error: urllib.error.HTTPError) -> str:
    try:
        response = error.read().decode("utf-8", errors="replace")
    finally:
        error.close()
    try:
        body = json.loads(response)
        errors = body.get("errors", [])
        return "; ".join(item["detail"] for item in errors)
    except (json.JSONDecodeError, AttributeError, KeyError, TypeError):
        return response


class CratesIoClient:
    def __init__(self, registry_url: str = REGISTRY_URL, token: str | None = None):
        self.registry_url = registry_url.rstrip("/")
        self.token = token

    def _request(
        self,
        method: str,
        path: str,
        operation: str,
        body: dict[str, object] | None = None,
        authenticated: bool = False,
    ) -> dict[str, object]:
        headers = {
            "Accept": "application/json",
            "User-Agent": USER_AGENT,
        }
        if authenticated:
            if not self.token:
                raise RuntimeError(f"{operation} requires a crates.io API token")
            headers["Authorization"] = self.token

        data = None
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"

        request = urllib.request.Request(
            f"{self.registry_url}{path}",
            data=data,
            headers=headers,
            method=method,
        )
        retryable = method in {"GET", "PATCH"}
        for attempt in range(5):
            try:
                with urllib.request.urlopen(request, timeout=30) as response:
                    return json.load(response)
            except urllib.error.HTTPError as error:
                should_retry = retryable and error.code in {429, 502, 503, 504}
                if should_retry and attempt < 4:
                    retry_after = (
                        error.headers.get("Retry-After") if error.headers else None
                    )
                    delay = (
                        int(retry_after)
                        if retry_after and retry_after.isdigit()
                        else 2**attempt
                    )
                    error.read()
                    error.close()
                    print(
                        f"{operation} returned HTTP {error.code}; retrying in {delay}s",
                        flush=True,
                    )
                    time.sleep(delay)
                    continue
                raise ApiError(
                    operation, error.code, _api_error_detail(error)
                ) from None
            except urllib.error.URLError as error:
                if retryable and attempt < 4:
                    delay = 2**attempt
                    print(
                        f"{operation} failed: {error.reason}; retrying in {delay}s",
                        flush=True,
                    )
                    time.sleep(delay)
                    continue
                raise RuntimeError(f"{operation} failed: {error.reason}") from None

        raise AssertionError("unreachable")

    def get_crate(self, name: str) -> dict[str, object] | None:
        encoded_name = urllib.parse.quote(name, safe="")
        try:
            response = self._request(
                "GET",
                f"/api/v1/crates/{encoded_name}",
                f"reading crate {name}",
            )
        except ApiError as error:
            if error.status == 404:
                return None
            raise
        krate = response.get("crate")
        if not isinstance(krate, dict):
            raise RuntimeError(f"crates.io returned invalid metadata for {name}")
        return krate

    def get_version(self, name: str, version: str) -> dict[str, object]:
        encoded_name = urllib.parse.quote(name, safe="")
        encoded_version = urllib.parse.quote(version, safe="")
        response = self._request(
            "GET",
            f"/api/v1/crates/{encoded_name}/{encoded_version}",
            f"reading {name} {version}",
        )
        version_data = response.get("version")
        if not isinstance(version_data, dict):
            raise RuntimeError(
                f"crates.io returned invalid version metadata for {name} {version}"
            )
        return version_data

    def list_github_configs(self, name: str) -> list[dict[str, object]]:
        query = urllib.parse.urlencode({"crate": name, "per_page": 100})
        response = self._request(
            "GET",
            f"/api/v1/trusted_publishing/github_configs?{query}",
            f"listing Trusted Publishers for {name}",
            authenticated=True,
        )
        configs = response.get("github_configs")
        if not isinstance(configs, list) or not all(
            isinstance(config, dict) for config in configs
        ):
            raise RuntimeError(
                f"crates.io returned invalid Trusted Publisher metadata for {name}"
            )
        return configs

    def create_github_config(self, name: str) -> dict[str, object]:
        config = {"crate": name, **PUBLISHER}
        response = self._request(
            "POST",
            "/api/v1/trusted_publishing/github_configs",
            f"creating the Trusted Publisher for {name}",
            body={"github_config": config},
            authenticated=True,
        )
        created = response.get("github_config")
        if not isinstance(created, dict):
            raise RuntimeError(
                f"crates.io returned invalid Trusted Publisher metadata for {name}"
            )
        return created

    def set_trustpub_only(self, name: str) -> dict[str, object]:
        encoded_name = urllib.parse.quote(name, safe="")
        response = self._request(
            "PATCH",
            f"/api/v1/crates/{encoded_name}",
            f"enabling Trusted Publishing only for {name}",
            body={"crate": {"trustpub_only": True}},
            authenticated=True,
        )
        krate = response.get("crate")
        if not isinstance(krate, dict):
            raise RuntimeError(f"crates.io returned invalid metadata for {name}")
        return krate


def _normalized_repository(value: object) -> str:
    if not isinstance(value, str):
        return ""
    normalized = value.rstrip("/")
    if normalized.endswith(".git"):
        normalized = normalized[:-4]
    return normalized.lower()


def validate_crate_metadata(
    planned: PlannedCrate, metadata: dict[str, object], client: CratesIoClient
) -> None:
    if metadata.get("id") != planned.name:
        raise RuntimeError(
            f"crate name mismatch for {planned.name}: got {metadata.get('id')!r}"
        )
    if _normalized_repository(metadata.get("repository")) != _normalized_repository(
        REPOSITORY
    ):
        raise RuntimeError(
            f"{planned.name} already exists with an unexpected repository: "
            f"{metadata.get('repository')!r}"
        )

    if metadata.get("max_version") == PLACEHOLDER_VERSION:
        if metadata.get("description") != PLACEHOLDER_DESCRIPTION:
            raise RuntimeError(
                f"{planned.name} has an unexpected {PLACEHOLDER_VERSION} placeholder"
            )
        version = client.get_version(planned.name, PLACEHOLDER_VERSION)
        if version.get("num") != PLACEHOLDER_VERSION:
            raise RuntimeError(
                f"{planned.name} placeholder version could not be verified"
            )


def _is_expected_config(name: str, config: dict[str, object]) -> bool:
    repository_owner = config.get("repository_owner")
    repository_name = config.get("repository_name")
    return all(
        (
            config.get("crate") == name,
            isinstance(repository_owner, str)
            and repository_owner.lower() == PUBLISHER["repository_owner"],
            isinstance(repository_name, str)
            and repository_name.lower() == PUBLISHER["repository_name"],
            config.get("workflow_filename") == PUBLISHER["workflow_filename"],
            config.get("environment") == PUBLISHER["environment"],
        )
    )


def validate_github_configs(name: str, configs: list[dict[str, object]]) -> None:
    if len(configs) != 1 or not _is_expected_config(name, configs[0]):
        compact = [
            {
                key: config.get(key)
                for key in (
                    "repository_owner",
                    "repository_name",
                    "workflow_filename",
                    "environment",
                )
            }
            for config in configs
        ]
        raise RuntimeError(
            f"{name} has unexpected Trusted Publisher configurations: "
            f"{json.dumps(compact, sort_keys=True)}"
        )


def preflight_authenticated(
    packages: list[PlannedCrate],
    candidate_names: set[str],
    client: CratesIoClient,
) -> list[str]:
    verified: list[str] = []
    for planned in packages:
        metadata = client.get_crate(planned.name)
        is_candidate = planned.name in candidate_names
        if metadata is None:
            if not is_candidate:
                raise RuntimeError(
                    f"{planned.name} is missing but was not selected for bootstrap"
                )
            verified.append(planned.name)
            continue

        validate_crate_metadata(planned, metadata, client)
        is_placeholder = metadata.get("max_version") == PLACEHOLDER_VERSION
        if is_placeholder != is_candidate:
            state = "a placeholder" if is_placeholder else "an established crate"
            raise RuntimeError(
                f"{planned.name} is now {state}, which does not match discovery"
            )

        configs = client.list_github_configs(planned.name)
        if is_placeholder:
            if configs:
                validate_github_configs(planned.name, configs)
        else:
            validate_github_configs(planned.name, configs)
            if metadata.get("trustpub_only") is not True:
                raise RuntimeError(
                    f"{planned.name} does not require Trusted Publishing"
                )
        verified.append(planned.name)
    return verified


def verify_authenticated(
    packages: list[PlannedCrate], client: CratesIoClient
) -> list[str]:
    verified: list[str] = []
    for planned in packages:
        metadata = client.get_crate(planned.name)
        if metadata is None:
            raise RuntimeError(f"{planned.name} does not exist on crates.io")
        validate_crate_metadata(planned, metadata, client)
        validate_github_configs(planned.name, client.list_github_configs(planned.name))
        if metadata.get("trustpub_only") is not True:
            raise RuntimeError(f"{planned.name} does not require Trusted Publishing")
        verified.append(planned.name)
    return verified


def _placeholder_manifest(name: str) -> str:
    return f"""# Licensed to the Apache Software Foundation (ASF) under one
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

[package]
name = {json.dumps(name)}
version = {json.dumps(PLACEHOLDER_VERSION)}
edition = "2024"
rust-version = "1.91"
description = {json.dumps(PLACEHOLDER_DESCRIPTION)}
homepage = "https://opendal.apache.org/"
repository = {json.dumps(REPOSITORY)}
license = "Apache-2.0"
readme = "README.md"
include = ["src/lib.rs", "README.md", "LICENSE", "NOTICE"]
"""


def _placeholder_readme(name: str) -> str:
    return f"""# {name}

This crate belongs to [Apache OpenDAL]({REPOSITORY}).

Version {PLACEHOLDER_VERSION} reserves the crates.io package name for Apache
OpenDAL. It is not an ASF software release, contains no implementation, and must
not be used as a dependency.
"""


PLACEHOLDER_LIB = """// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

#![doc = include_str!("../README.md")]
"""


def write_placeholder_package(
    project_dir: Path, planned: PlannedCrate, package_dir: Path
) -> None:
    source_dir = package_dir / "src"
    source_dir.mkdir()
    (package_dir / "Cargo.toml").write_text(
        _placeholder_manifest(planned.name), encoding="utf-8"
    )
    (package_dir / "README.md").write_text(
        _placeholder_readme(planned.name), encoding="utf-8"
    )
    (source_dir / "lib.rs").write_text(PLACEHOLDER_LIB, encoding="utf-8")
    shutil.copyfile(project_dir / "LICENSE", package_dir / "LICENSE")
    shutil.copyfile(project_dir / "NOTICE", package_dir / "NOTICE")


def publish_placeholder(project_dir: Path, planned: PlannedCrate, token: str) -> None:
    with tempfile.TemporaryDirectory(prefix=f"{planned.name}-bootstrap-") as tmpdir:
        package_dir = Path(tmpdir)
        write_placeholder_package(project_dir, planned, package_dir)

        env = os.environ.copy()
        env["CARGO_REGISTRY_TOKEN"] = token
        command = ["cargo", "publish", "--manifest-path", "Cargo.toml"]
        while True:
            process = subprocess.run(
                command,
                cwd=package_dir,
                env=env,
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
            )
            output = process.stdout or ""
            print(output, end="", flush=True)
            if process.returncode == 0:
                return
            if should_retry(output):
                delay = parse_retry_after(output)
                print(
                    f"crates.io rate limited {planned.name}; sleeping {delay}s",
                    flush=True,
                )
                time.sleep(delay)
                continue
            raise subprocess.CalledProcessError(
                process.returncode, command, output=output
            )


def wait_for_crate(
    client: CratesIoClient, name: str, timeout: int = 120
) -> dict[str, object]:
    deadline = time.monotonic() + timeout
    while True:
        metadata = client.get_crate(name)
        if metadata is not None:
            return metadata
        if time.monotonic() >= deadline:
            raise RuntimeError(
                f"timed out waiting for {name} to become visible on crates.io"
            )
        time.sleep(2)


def wait_for_trustpub_only(
    client: CratesIoClient, name: str, timeout: int = 120
) -> dict[str, object]:
    deadline = time.monotonic() + timeout
    while True:
        metadata = client.get_crate(name)
        if metadata is not None and metadata.get("trustpub_only") is True:
            return metadata
        if time.monotonic() >= deadline:
            raise RuntimeError(
                f"timed out waiting for {name} to require Trusted Publishing"
            )
        time.sleep(2)


def wait_for_expected_config(
    client: CratesIoClient, name: str, timeout: int = 120
) -> None:
    deadline = time.monotonic() + timeout
    while True:
        configs = client.list_github_configs(name)
        if configs:
            validate_github_configs(name, configs)
            return
        if time.monotonic() >= deadline:
            raise RuntimeError(
                f"timed out waiting for the Trusted Publisher for {name}"
            )
        time.sleep(2)


def reconcile_crate(
    project_dir: Path,
    planned: PlannedCrate,
    client: CratesIoClient,
    token: str,
) -> ReconcileResult:
    actions: list[str] = []
    metadata = client.get_crate(planned.name)
    if metadata is None:
        publish_placeholder(project_dir, planned, token)
        metadata = wait_for_crate(client, planned.name)
        actions.append("created placeholder")
    elif metadata.get("max_version") != PLACEHOLDER_VERSION:
        raise RuntimeError(
            f"{planned.name} became an established crate after discovery; "
            "refusing to modify it in the bootstrap workflow"
        )

    validate_crate_metadata(planned, metadata, client)

    configs = client.list_github_configs(planned.name)
    if not configs:
        created = client.create_github_config(planned.name)
        if not _is_expected_config(planned.name, created):
            raise RuntimeError(
                f"crates.io created an unexpected Trusted Publisher for {planned.name}"
            )
        actions.append("configured Trusted Publishing")
    else:
        validate_github_configs(planned.name, configs)

    if metadata.get("trustpub_only") is not True:
        updated = client.set_trustpub_only(planned.name)
        if updated.get("trustpub_only") is not True:
            raise RuntimeError(
                f"crates.io did not enable Trusted Publishing only for {planned.name}"
            )
        actions.append("enabled Trusted Publishing only")

    verified_metadata = wait_for_trustpub_only(client, planned.name)
    validate_crate_metadata(planned, verified_metadata, client)
    wait_for_expected_config(client, planned.name)

    if not actions:
        actions.append("verified")
    return ReconcileResult(planned.name, tuple(actions))


def discover(
    project_dir: Path, client: CratesIoClient
) -> tuple[list[PlannedCrate], list[str], list[str]]:
    packages = planned_crates(project_dir)
    missing: list[str] = []
    placeholders: list[str] = []
    for planned in packages:
        metadata = client.get_crate(planned.name)
        if metadata is None:
            missing.append(planned.name)
            continue
        validate_crate_metadata(planned, metadata, client)
        if metadata.get("max_version") == PLACEHOLDER_VERSION:
            placeholders.append(planned.name)
    return packages, missing, placeholders


def verify_public(project_dir: Path, client: CratesIoClient) -> list[str]:
    verified: list[str] = []
    for planned in planned_crates(project_dir):
        metadata = client.get_crate(planned.name)
        if metadata is None:
            raise RuntimeError(f"{planned.name} does not exist on crates.io")
        validate_crate_metadata(planned, metadata, client)
        if metadata.get("trustpub_only") is not True:
            raise RuntimeError(f"{planned.name} does not require Trusted Publishing")
        verified.append(planned.name)
    return verified


def _write_summary(title: str, lines: list[str]) -> None:
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary_path:
        return
    source_commit = os.environ.get("GITHUB_SHA", "unknown")
    with Path(summary_path).open("a", encoding="utf-8") as summary:
        summary.write(f"## {title}\n\n")
        summary.write(f"Source commit: `{source_commit}`\n\n")
        for line in lines:
            summary.write(f"- {line}\n")
        summary.write("\n")


def run_discover(args: argparse.Namespace) -> int:
    client = CratesIoClient(args.registry_url)
    packages, missing, placeholders = discover(args.project_dir, client)
    candidates = [*missing, *placeholders]
    result = {
        "packages": len(packages),
        "missing": missing,
        "placeholders": placeholders,
        "bootstrap_candidates": candidates,
    }
    print(json.dumps(result, indent=2))
    _write_summary(
        "Rust crate bootstrap discovery",
        [
            f"Publishable crates: {len(packages)}",
            f"Missing crates: {', '.join(missing) if missing else 'none'}",
            f"Placeholder crates: {', '.join(placeholders) if placeholders else 'none'}",
        ],
    )
    return 0


def run_apply(args: argparse.Namespace) -> int:
    token = os.environ.get("CARGO_REGISTRY_BOOTSTRAP_TOKEN")
    if not token:
        raise RuntimeError("CARGO_REGISTRY_BOOTSTRAP_TOKEN is not set")

    client = CratesIoClient(args.registry_url, token=token)
    packages, missing, placeholders = discover(args.project_dir, client)
    candidate_set = {*missing, *placeholders}
    candidates = [package for package in packages if package.name in candidate_set]

    results: list[ReconcileResult] = []
    authenticated: list[str] = []
    try:
        preflight_authenticated(packages, candidate_set, client)
        print(
            f"authenticated preflight passed for {len(packages)} planned crates",
            flush=True,
        )
        print(
            f"bootstrap candidates: {len(candidates)}",
            flush=True,
        )
        for planned in candidates:
            result = reconcile_crate(args.project_dir, planned, client, token)
            results.append(result)
            print(f"{result.name}: {', '.join(result.actions)}", flush=True)
        authenticated = verify_authenticated(packages, client)
        print(
            f"authenticated final audit passed for {len(authenticated)} planned crates",
            flush=True,
        )
    except Exception as error:
        _write_summary(
            "Rust crate bootstrap",
            [
                *(
                    f"`{result.name}`: {', '.join(result.actions)}"
                    for result in results
                ),
                f"Failed: {error}",
            ],
        )
        raise

    _write_summary(
        "Rust crate bootstrap",
        [
            f"Authenticated preflight: {len(packages)} planned crates",
            *(f"`{result.name}`: {', '.join(result.actions)}" for result in results),
            f"Authenticated final audit: {len(authenticated)} planned crates",
        ],
    )
    return 0


def run_verify(args: argparse.Namespace) -> int:
    verified = verify_public(args.project_dir, CratesIoClient(args.registry_url))
    print(json.dumps({"verified": verified}, indent=2))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Create and secure crates.io names in the OpenDAL Rust publish plan."
    )
    parser.add_argument(
        "--project-dir",
        type=Path,
        default=PROJECT_DIR,
        help="Path to the repository root.",
    )
    parser.add_argument(
        "--registry-url",
        default=REGISTRY_URL,
        help="crates.io-compatible registry API URL.",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    discover_parser = subparsers.add_parser(
        "discover", help="Report missing and placeholder crate names."
    )
    discover_parser.set_defaults(run=run_discover)

    apply_parser = subparsers.add_parser(
        "apply",
        help="Audit all planned crates and reconcile missing names and placeholders.",
    )
    apply_parser.set_defaults(run=run_apply)

    verify_parser = subparsers.add_parser(
        "verify", help="Verify public crate existence and Trusted Publishing only."
    )
    verify_parser.set_defaults(run=run_verify)

    args = parser.parse_args()
    args.project_dir = args.project_dir.resolve()
    return args.run(args)


if __name__ == "__main__":
    raise SystemExit(main())
