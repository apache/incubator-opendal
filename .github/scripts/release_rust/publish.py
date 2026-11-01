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
import re
import subprocess
import time
import tomllib
from collections.abc import Iterable
from contextlib import nullcontext
from dataclasses import dataclass
from datetime import datetime
from datetime import timezone
from email.utils import parsedate_to_datetime
from pathlib import Path

from trusted_publishing import temporary_trusted_publishing_token


SCRIPT_PATH = Path(__file__).resolve()
PROJECT_DIR = SCRIPT_PATH.parents[3]


@dataclass(frozen=True)
class Patch:
    manifest_path: Path
    original_manifest: str
    lockfiles: dict[Path, str]


def load_manifest(manifest_path: Path) -> dict:
    with manifest_path.open("rb") as fp:
        return tomllib.load(fp)


def package_name(manifest_path: Path) -> str:
    manifest = load_manifest(manifest_path)
    return manifest["package"]["name"]


def collect_lockfiles(project_dir: Path) -> dict[Path, str]:
    lockfiles: dict[Path, str] = {}
    for path in project_dir.rglob("Cargo.lock"):
        if "target" in path.parts:
            continue
        lockfiles[path] = path.read_text()
    return lockfiles


def iter_dependency_names(table: dict | None, manifest_dir: Path) -> Iterable[str]:
    if not isinstance(table, dict):
        return

    for name, dependency in table.items():
        if not isinstance(dependency, dict):
            continue
        path = dependency.get("path")
        if not isinstance(path, str):
            continue
        if (manifest_dir / path).resolve().is_dir():
            yield name


def local_dev_dependency_names(manifest: dict, manifest_dir: Path) -> set[str]:
    names = set(iter_dependency_names(manifest.get("dev-dependencies"), manifest_dir))

    for target in manifest.get("target", {}).values():
        if not isinstance(target, dict):
            continue
        names.update(iter_dependency_names(target.get("dev-dependencies"), manifest_dir))

    return names


def section_header(line: str) -> str | None:
    stripped = line.strip()
    if stripped.startswith("[") and stripped.endswith("]"):
        return stripped
    return None


def is_dev_dependency_section(header: str | None) -> bool:
    return header == "[dev-dependencies]" or (
        header is not None and header.startswith("[target.") and header.endswith(".dev-dependencies]")
    )


def remove_dependency_entry(lines: list[str], start: int) -> int:
    brace_depth = lines[start].count("{") - lines[start].count("}")
    bracket_depth = lines[start].count("[") - lines[start].count("]")
    end = start + 1

    while end < len(lines) and (brace_depth > 0 or bracket_depth > 0):
        brace_depth += lines[end].count("{") - lines[end].count("}")
        bracket_depth += lines[end].count("[") - lines[end].count("]")
        end += 1

    del lines[start:end]
    return start


def strip_local_dev_dependencies(manifest_path: Path, dependency_names: set[str]) -> bool:
    if not dependency_names:
        return False

    lines = manifest_path.read_text().splitlines(keepends=True)
    header = None
    changed = False
    index = 0

    while index < len(lines):
        current_header = section_header(lines[index])
        if current_header is not None:
            header = current_header
            index += 1
            continue

        if is_dev_dependency_section(header):
            match = re.match(r"^([A-Za-z0-9_-]+)\s*=", lines[index])
            if match and match.group(1) in dependency_names:
                index = remove_dependency_entry(lines, index)
                changed = True
                continue

        index += 1

    if changed:
        manifest_path.write_text("".join(lines))
    return changed


def prepare_manifest(project_dir: Path, package_dir: Path) -> Patch | None:
    manifest_path = package_dir / "Cargo.toml"
    manifest = load_manifest(manifest_path)
    dependency_names = local_dev_dependency_names(manifest, package_dir)
    if not dependency_names:
        return None

    patch = Patch(
        manifest_path=manifest_path,
        original_manifest=manifest_path.read_text(),
        lockfiles=collect_lockfiles(project_dir),
    )
    changed = strip_local_dev_dependencies(manifest_path, dependency_names)
    return patch if changed else None


def restore(patch: Patch | None) -> None:
    if patch is None:
        return

    patch.manifest_path.write_text(patch.original_manifest)
    for path, content in patch.lockfiles.items():
        path.write_text(content)


def parse_retry_after(output: str) -> int:
    match = re.search(r"Please try again after ([^\n]+)", output)
    if match:
        value = match.group(1).strip().rstrip(".")
        for parser in (
            lambda text: parsedate_to_datetime(text),
            lambda text: datetime.fromisoformat(text.replace("Z", "+00:00")),
        ):
            try:
                retry_at = parser(value)
                if retry_at.tzinfo is None:
                    retry_at = retry_at.replace(tzinfo=timezone.utc)
                return max(60, int((retry_at - datetime.now(timezone.utc)).total_seconds()) + 8)
            except ValueError:
                continue

    return 610


def should_retry(output: str) -> bool:
    lowered = output.lower()
    return (
        "too many requests" in lowered
        or "rate limit" in lowered
        or "you have published too many crates" in lowered
    )


def already_published(output: str) -> bool:
    lowered = output.lower()
    return "already uploaded" in lowered or "already exists" in lowered or "is already uploaded" in lowered


def publish_package(project_dir: Path, package: str, dry_run: bool) -> None:
    package_dir = project_dir / package
    name = package_name(package_dir / "Cargo.toml")

    while True:
        print(f"Publishing {name} from {package}", flush=True)
        patch = prepare_manifest(project_dir, package_dir)
        try:
            cmd = ["cargo", "publish", "--package", name, "--no-verify"]
            if dry_run:
                cmd.append("--dry-run")
            if patch is not None:
                cmd.append("--allow-dirty")

            token_context = (
                nullcontext(None) if dry_run else temporary_trusted_publishing_token()
            )
            with token_context as token:
                env = os.environ.copy()
                if token is None:
                    env.pop("CARGO_REGISTRY_TOKEN", None)
                else:
                    env["CARGO_REGISTRY_TOKEN"] = token

                proc = subprocess.run(
                    cmd,
                    cwd=package_dir,
                    check=False,
                    env=env,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                )

                output = proc.stdout or ""
                print(output, end="", flush=True)
        finally:
            restore(patch)

        if proc.returncode == 0:
            return
        if already_published(output):
            print(f"Skipping {name}: already published", flush=True)
            return
        if should_retry(output):
            sleep_for = parse_retry_after(output)
            print(f"crates.io rate limited {name}; sleeping {sleep_for}s", flush=True)
            time.sleep(sleep_for)
            continue

        raise subprocess.CalledProcessError(proc.returncode, cmd, output=output)


def main() -> int:
    parser = argparse.ArgumentParser(description="Publish Rust crates for an OpenDAL release.")
    parser.add_argument(
        "--project-dir",
        type=Path,
        default=PROJECT_DIR,
        help="Path to the repository root.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Run cargo publish --dry-run for every package without uploading.",
    )
    args = parser.parse_args()

    packages = json.loads(os.environ["PACKAGES"])
    project_dir = args.project_dir.resolve()
    for package in packages:
        publish_package(project_dir, package, args.dry_run)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
