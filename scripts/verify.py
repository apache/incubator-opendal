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

import subprocess
import sys
import os
from pathlib import Path

BASE_DIR = Path(os.getcwd())

# Define colors for output
YELLOW = "\033[37;1m"
GREEN = "\033[32;1m"
ENDCOLOR = "\033[0m"


def check_signature(pkg):
    """Check the GPG signature of the package."""
    try:
        subprocess.check_call(["gpg", "--verify", f"{pkg}.asc", pkg])
        print(f"{GREEN}> Success to verify the gpg sign for {pkg}{ENDCOLOR}")
    except subprocess.CalledProcessError:
        print(f"{YELLOW}> Failed to verify the gpg sign for {pkg}{ENDCOLOR}")


def check_sha512sum(pkg):
    """Check the sha512 checksum of the package."""
    try:
        subprocess.check_call(["shasum", "-a", "512", "-c", f"{pkg}.sha512"])
        print(f"{GREEN}> Success to verify the checksum for {pkg}{ENDCOLOR}")
    except subprocess.CalledProcessError:
        print(f"{YELLOW}> Failed to verify the checksum for {pkg}{ENDCOLOR}")


def extract_packages():
    for file in BASE_DIR.glob("*.tar.gz"):
        subprocess.run(["tar", "-xzf", file], check=True)


def check_license(dir):
    print(f"> Start checking LICENSE file in {dir}")
    if not (dir / "LICENSE").exists():
        raise f"{YELLOW}> LICENSE file is not found{ENDCOLOR}"
    print(f"{GREEN}> LICENSE file exists in {dir}{ENDCOLOR}")


def check_notice(dir):
    print(f"> Start checking NOTICE file in {dir}")
    if not (dir / "NOTICE").exists():
        raise f"{YELLOW}> NOTICE file is not found{ENDCOLOR}"
    print(f"{GREEN}> NOTICE file exists in {dir}{ENDCOLOR}")


def check_rust():
    try:
        subprocess.run(["cargo", "--version"], check=True)
        return True
    except FileNotFoundError:
        return False
    except Exception as e:
        raise Exception("Check rust met unexpected error", e)


def check_java():
    try:
        subprocess.run(["java", "-version"], check=True)
        return True
    except FileNotFoundError:
        return False
    except Exception as e:
        raise Exception("Check java met unexpected error", e)


def build_core(dir):
    print("Start building opendal core", flush=True)

    subprocess.run(
        ["cargo", "build", "--release"],
        cwd=dir / "core",
        check=True,
    )
    print(f"{GREEN}Success to build opendal core{ENDCOLOR}")


def build_java_binding(dir):
    print("Start building opendal java binding", flush=True)

    subprocess.run(
        [
            "./mvnw",
            "clean",
            "install",
            "-DskipTests=true",
            "-Dcargo-build.profile=release",
        ],
        check=True,
        cwd=dir / "bindings/java",
    )
    print(f"> {GREEN}Success to build opendal java binding{ENDCOLOR}")


if __name__ == "__main__":
    # Get a list of all files in the current directory
    files = [f for f in os.listdir(".") if os.path.isfile(f)]

    for pkg in files:
        # Skip files that don't have a corresponding .asc or .sha512 file
        if not os.path.exists(f"{pkg}.asc") or not os.path.exists(f"{pkg}.sha512"):
            continue

        print(f"> Checking {pkg}")

        # Perform the checks
        check_signature(pkg)
        check_sha512sum(pkg)

    extract_packages()

    for dir in BASE_DIR.glob("apache-opendal-*-src/"):
        check_license(dir)
        check_notice(dir)

        if "core" in dir.name:
            if check_rust():
                build_core(dir)
            else:
                print(
                    "Cargo is not found, please check if rust development has been setup correctly"
                )
                print(
                    "Visit https://www.rust-lang.org/tools/install for more information"
                )
                sys.exit(1)

        if "java" in dir.name:
            if check_java():
                build_java_binding(dir)
            else:
                print("Java is not found, skipped building java binding")
