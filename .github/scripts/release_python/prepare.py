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
import os
import re
import tomllib
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve()
PROJECT_DIR = SCRIPT_PATH.parents[3]
PYTHON_BINDING_DIR = PROJECT_DIR / "bindings" / "python"
DEFAULT_CARGO_MANIFEST = PYTHON_BINDING_DIR / "Cargo.toml"
DEFAULT_PYPROJECT = PYTHON_BINDING_DIR / "pyproject.toml"

RC_TAG_PATTERN = re.compile(
    r"^v(?P<release>[0-9]+\.[0-9]+\.[0-9]+)-rc\.(?P<candidate>[1-9][0-9]*)$"
)
STABLE_VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
PROJECT_SECTION_PATTERN = re.compile(
    r"(?ms)^\[project\][^\S\r\n]*(?:\r?\n)(?P<body>.*?)(?=^\[|\Z)"
)
DYNAMIC_VERSION_PATTERN = re.compile(
    r"(?m)^(?P<indent>[ \t]*)dynamic[ \t]*=[ \t]*"
    r"\[[^\r\n]*\][ \t]*(?:#[^\r\n]*)?(?P<newline>\r?\n|\Z)"
)


class PreparationError(ValueError):
    pass


def load_stable_version(cargo_manifest: Path) -> str:
    with cargo_manifest.open("rb") as fp:
        manifest = tomllib.load(fp)

    package = manifest.get("package")
    version = package.get("version") if isinstance(package, dict) else None
    if not isinstance(version, str) or not STABLE_VERSION_PATTERN.fullmatch(version):
        raise PreparationError(
            f"{cargo_manifest} must define a stable package version in X.Y.Z form"
        )

    return version


def release_candidate_version(stable_version: str, tag: str) -> str:
    match = RC_TAG_PATTERN.fullmatch(tag)
    if match is None:
        raise PreparationError(
            f"tag {tag!r} must use the vX.Y.Z-rc.N release candidate format"
        )

    return f"{stable_version}rc{match.group('candidate')}"


def render_pyproject(content: str, version: str) -> str:
    metadata = tomllib.loads(content)
    project = metadata.get("project")
    if not isinstance(project, dict):
        raise PreparationError("pyproject.toml must define a [project] table")

    dynamic = project.get("dynamic")
    current_version = project.get("version")
    if current_version is not None:
        if current_version != version or dynamic is not None:
            raise PreparationError(
                "pyproject.toml already defines unexpected project version metadata"
            )
        return content

    if dynamic != ["version"]:
        raise PreparationError(
            "pyproject.toml must declare only version as dynamic project metadata"
        )

    project_section = PROJECT_SECTION_PATTERN.search(content)
    if project_section is None:
        raise PreparationError("could not locate the [project] table")

    body = project_section.group("body")
    assignments = list(DYNAMIC_VERSION_PATTERN.finditer(body))
    if len(assignments) != 1:
        raise PreparationError(
            "the [project] dynamic version declaration must be on one line"
        )

    assignment = assignments[0]
    replacement = (
        f'{assignment.group("indent")}version = "{version}"'
        f"{assignment.group('newline')}"
    )
    rendered_body = body[: assignment.start()] + replacement + body[assignment.end() :]
    return (
        content[: project_section.start("body")]
        + rendered_body
        + content[project_section.end("body") :]
    )


def prepare(
    tag: str,
    cargo_manifest: Path = DEFAULT_CARGO_MANIFEST,
    pyproject: Path = DEFAULT_PYPROJECT,
) -> str:
    stable_version = load_stable_version(cargo_manifest)
    version = release_candidate_version(stable_version, tag)

    content = pyproject.read_text(encoding="utf-8")
    rendered = render_pyproject(content, version)
    if rendered != content:
        pyproject.write_text(rendered, encoding="utf-8")

    return version


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Prepare PEP 440 version metadata for a Python release candidate."
    )
    parser.add_argument(
        "--tag",
        default=os.environ.get("GITHUB_REF_NAME"),
        help="Release candidate tag. Defaults to GITHUB_REF_NAME.",
    )
    parser.add_argument(
        "--cargo-manifest",
        type=Path,
        default=DEFAULT_CARGO_MANIFEST,
        help="Path to the Python binding Cargo.toml.",
    )
    parser.add_argument(
        "--pyproject",
        type=Path,
        default=DEFAULT_PYPROJECT,
        help="Path to the Python binding pyproject.toml.",
    )
    args = parser.parse_args()

    if not args.tag:
        parser.error("--tag is required when GITHUB_REF_NAME is not set")

    try:
        version = prepare(args.tag, args.cargo_manifest, args.pyproject)
    except (OSError, PreparationError, tomllib.TOMLDecodeError) as error:
        parser.error(str(error))

    print(f"Prepared Python release candidate version {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
