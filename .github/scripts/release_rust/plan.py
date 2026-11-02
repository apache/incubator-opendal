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
import tomllib
from collections import defaultdict, deque
from dataclasses import dataclass
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve()
PROJECT_DIR = SCRIPT_PATH.parents[3]
PUBLISH_GLOBS = (
    "core/Cargo.toml",
    "core/core/Cargo.toml",
    "core/testkit/Cargo.toml",
    "core/http-transports/*/Cargo.toml",
    "core/layers/*/Cargo.toml",
    "core/services/*/Cargo.toml",
    "integrations/*/Cargo.toml",
)


@dataclass(frozen=True)
class Package:
    manifest_path: Path
    manifest_dir: Path
    path: str


def discover_publishable_packages(project_dir: Path) -> dict[Path, Package]:
    packages: dict[Path, Package] = {}

    for pattern in PUBLISH_GLOBS:
        for manifest_path in sorted(project_dir.glob(pattern)):
            with manifest_path.open("rb") as fp:
                manifest = tomllib.load(fp)

            package = manifest.get("package")
            if not package or package.get("publish") is False:
                continue

            manifest_dir = manifest_path.parent.resolve()
            packages[manifest_dir] = Package(
                manifest_path=manifest_path.resolve(),
                manifest_dir=manifest_dir,
                path=manifest_dir.relative_to(project_dir).as_posix(),
            )

    return packages


def iter_local_dependencies(manifest: dict, manifest_dir: Path) -> list[Path]:
    dependencies: list[Path] = []

    def visit_table(table: dict | None) -> None:
        if not isinstance(table, dict):
            return

        for dependency in table.values():
            if not isinstance(dependency, dict):
                continue
            path = dependency.get("path")
            if not isinstance(path, str):
                continue
            dependencies.append((manifest_dir / path).resolve())

    for name in ("dependencies", "build-dependencies"):
        visit_table(manifest.get(name))

    for target in manifest.get("target", {}).values():
        if not isinstance(target, dict):
            continue
        for name in ("dependencies", "build-dependencies"):
            visit_table(target.get(name))

    return dependencies


def plan(project_dir: Path = PROJECT_DIR) -> list[str]:
    project_dir = project_dir.resolve()
    packages = discover_publishable_packages(project_dir)

    graph: dict[Path, set[Path]] = defaultdict(set)
    indegree = {manifest_dir: 0 for manifest_dir in packages}

    for manifest_dir, package in packages.items():
        with package.manifest_path.open("rb") as fp:
            manifest = tomllib.load(fp)

        for dependency_dir in iter_local_dependencies(manifest, manifest_dir):
            if dependency_dir not in packages:
                continue
            if manifest_dir in graph[dependency_dir]:
                continue

            graph[dependency_dir].add(manifest_dir)
            indegree[manifest_dir] += 1

    queue = deque(
        sorted(
            (manifest_dir for manifest_dir, degree in indegree.items() if degree == 0),
            key=lambda manifest_dir: packages[manifest_dir].path,
        )
    )

    ordered: list[str] = []
    while queue:
        manifest_dir = queue.popleft()
        ordered.append(packages[manifest_dir].path)

        for dependent in sorted(
            graph[manifest_dir], key=lambda dependent: packages[dependent].path
        ):
            indegree[dependent] -= 1
            if indegree[dependent] == 0:
                queue.append(dependent)

    if len(ordered) != len(packages):
        raise RuntimeError("failed to resolve publish order for Rust crates")

    return ordered


def write_github_output(packages: list[str]) -> None:
    github_output = os.environ.get("GITHUB_OUTPUT")
    if not github_output:
        raise RuntimeError("GITHUB_OUTPUT is not set")

    with Path(github_output).open("a", encoding="utf-8") as fp:
        fp.write(f"packages={json.dumps(packages)}\n")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Plan the publish order for Rust crates released from this repository."
    )
    parser.add_argument(
        "--project-dir",
        type=Path,
        default=PROJECT_DIR,
        help="Path to the repository root.",
    )
    parser.add_argument(
        "--github-output",
        action="store_true",
        help="Write the planned package list to GITHUB_OUTPUT as `packages=<json>`.",
    )
    args = parser.parse_args()

    packages = plan(args.project_dir)
    print(json.dumps(packages))

    if args.github_output:
        write_github_output(packages)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
