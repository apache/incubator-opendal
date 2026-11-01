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

import asyncio
from argparse import ArgumentParser, ArgumentDefaultsHelpFormatter
from contextlib import contextmanager
from pathlib import Path
import subprocess
from typing import Iterator

from constants import PACKAGE_FEATURES, PACKAGES


def is_tracked(path: Path) -> bool:
    result = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", path.name],
        cwd=path.parent,
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0


def cargo_deny_command(root: Path, *args: str, locked: bool = False) -> list[str]:
    command = ["cargo", "deny"]
    if locked:
        command.append("--locked")
    features = PACKAGE_FEATURES.get(root)
    if features:
        command.extend(["--features", ",".join(features)])
    command.extend(args)
    return command


@contextmanager
def resolved_dependencies(root: Path) -> Iterator[bool]:
    lockfile = root / "Cargo.lock"
    if is_tracked(lockfile):
        yield True
        return

    original_lockfile = lockfile.read_bytes() if lockfile.exists() else None
    lockfile.unlink(missing_ok=True)
    try:
        yield False
    finally:
        lockfile.unlink(missing_ok=True)
        if original_lockfile is not None:
            lockfile.write_bytes(original_lockfile)


def check_single_package(root: Path) -> None:
    if (root / "Cargo.toml").exists():
        print(f"Checking dependencies of {root}")
        with resolved_dependencies(root) as locked:
            subprocess.run(
                cargo_deny_command(root, "check", "licenses", locked=locked),
                cwd=root,
                check=True,
            )
    else:
        print(f"Skipping {root} as Cargo.toml does not exist")


def check_deps() -> None:
    failures: list[Path] = []
    for root in PACKAGES:
        # We disable dependencies checks since PHP binding release is not working.
        if str(root) in ["bindings/php"]:
            continue
        try:
            check_single_package(root)
        except subprocess.CalledProcessError:
            failures.append(root)
    if failures:
        roots = ", ".join(str(root) for root in failures)
        raise RuntimeError(f"dependency checks failed for: {roots}")


async def generate_single_package(root: Path) -> None:
    if (root / "Cargo.toml").exists():
        print(f"Generating dependencies {root}")
        with resolved_dependencies(root) as locked:
            command = cargo_deny_command(
                root, "list", "-f", "tsv", "-t", "0.6", locked=locked
            )
            process = await asyncio.create_subprocess_exec(
                *command, cwd=root, stdout=asyncio.subprocess.PIPE
            )
            stdout, _ = await process.communicate()
            if process.returncode != 0:
                raise subprocess.CalledProcessError(process.returncode, command)
            (root / "DEPENDENCIES.rust.tsv").write_bytes(stdout)
    else:
        print(f"Skipping {root} as Cargo.toml does not exist")


async def generate_all_deps() -> None:
    results = await asyncio.gather(
        *(generate_single_package(root) for root in PACKAGES),
        return_exceptions=True,
    )
    for result in results:
        if isinstance(result, BaseException):
            raise result


def generate_deps() -> None:
    asyncio.run(generate_all_deps())


if __name__ == "__main__":
    parser = ArgumentParser(formatter_class=ArgumentDefaultsHelpFormatter)
    parser.set_defaults(func=parser.print_help)
    subparsers = parser.add_subparsers()

    parser_check = subparsers.add_parser(
        "check", description="Check dependencies", help="Check dependencies"
    )
    parser_check.set_defaults(func=check_deps)

    parser_generate = subparsers.add_parser(
        "generate", description="Generate dependencies", help="Generate dependencies"
    )
    parser_generate.set_defaults(func=generate_deps)

    args = parser.parse_args()
    arg_dict = dict(vars(args))
    del arg_dict["func"]
    args.func(**arg_dict)
