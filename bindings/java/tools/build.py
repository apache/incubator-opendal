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

from argparse import ArgumentDefaultsHelpFormatter, ArgumentParser
from pathlib import Path
import glob
import shutil
import subprocess


def classifier_to_target(classifier: str) -> str:
    if classifier == "osx-aarch_64":
        return "aarch64-apple-darwin"
    if classifier == "osx-x86_64":
        return "x86_64-apple-darwin"
    if classifier == "linux-aarch_64":
        return "aarch64-unknown-linux-gnu"
    if classifier == "linux-aarch_64-musl":
        return "aarch64-unknown-linux-musl"
    if classifier == "linux-x86_64":
        return "x86_64-unknown-linux-gnu"
    if classifier == "linux-x86_64-musl":
        return "x86_64-unknown-linux-musl"
    if classifier == "windows-x86_64":
        return "x86_64-pc-windows-msvc"
    raise Exception(f"Unsupported classifier: {classifier}")


def get_cargo_artifact_name(classifier: str) -> str:
    if classifier.startswith("osx"):
        return "libopendal_java.dylib"
    if classifier.startswith("linux"):
        return "libopendal_java.so"
    if classifier.startswith("windows"):
        return "opendal_java.dll"
    raise Exception(f"Unsupported classifier: {classifier}")


def is_musl_runtime() -> bool:
    return len(glob.glob("/lib/ld-musl-*.so.1")) > 0 or len(glob.glob("/usr/lib/ld-musl-*.so.1")) > 0


if __name__ == "__main__":
    basedir = Path(__file__).parent.parent

    parser = ArgumentParser(formatter_class=ArgumentDefaultsHelpFormatter)
    parser.add_argument("--classifier", type=str, required=True)
    parser.add_argument("--target", type=str, default="")
    parser.add_argument("--profile", type=str, default="dev")
    parser.add_argument("--enable-zigbuild", type=str, default="false")
    args = parser.parse_args()

    if args.target:
        target = args.target
    else:
        target = classifier_to_target(args.classifier)

    # Setup target.
    command = ["rustup", "target", "add", target]
    print("$ " + subprocess.list2cmdline(command))
    subprocess.run(command, cwd=basedir, check=True)

    # Enable zigbuild if flag enabled and we are building linux gnu target.
    #
    # For musl targets, prefer using the system musl toolchain (e.g. `musl-tools` on Ubuntu)
    # instead of zigbuild.
    enable_zigbuild = args.enable_zigbuild == "true" and "linux" in target and not target.endswith("-musl")

    cmd = [
        "cargo",
        "zigbuild" if enable_zigbuild else "build",
        "--color=always",
        f"--profile={args.profile}",
    ]

    if enable_zigbuild and target.endswith("-gnu"):
        # Pin glibc to 2.17 if zigbuild has been enabled.
        cmd += ["--target", f"{target}.2.17"]
    else:
        cmd += ["--target", target]

    output = basedir / "target" / "bindings"
    Path(output).mkdir(exist_ok=True, parents=True)
    cmd += ["--target-dir", str(output)]

    print("$ " + subprocess.list2cmdline(cmd))
    subprocess.run(cmd, cwd=basedir, check=True)

    # History reason of cargo profiles.
    profile = "debug" if args.profile in ["dev", "test", "bench"] else args.profile

    artifact = get_cargo_artifact_name(args.classifier)
    src = output / target / profile / artifact
    dst = basedir / "target" / "classes" / "native" / args.classifier / artifact
    dst.parent.mkdir(exist_ok=True, parents=True)

    if target.endswith("-musl") and not is_musl_runtime():
        raise Exception(
            "Building musl artifacts requires running inside a musl environment (e.g. Alpine)."
        )

    shutil.copy2(src, dst)
