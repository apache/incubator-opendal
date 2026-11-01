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
import shutil
import subprocess


def classifier_to_target(classifier: str) -> str:
    if classifier == "osx-aarch_64":
        return "aarch64-apple-darwin"
    if classifier == "linux-x86_64":
        return "x86_64-unknown-linux-gnu"
    if classifier == "linux-aarch_64":
        return "aarch64-unknown-linux-gnu"
    if classifier == "windows-x86_64":
        return "x86_64-pc-windows-msvc"
    raise Exception(f"Unsupported classifier: {classifier}")


def get_cargo_artifact_name(classifier: str) -> str:
    if classifier == "osx-aarch_64":
        return "libopendal_dotnet.dylib"
    if classifier == "linux-x86_64" or classifier == "linux-aarch_64":
        return "libopendal_dotnet.so"
    if classifier == "windows-x86_64":
        return "opendal_dotnet.dll"
    raise Exception(f"Unsupported classifier: {classifier}")


if __name__ == "__main__":
    basedir = Path(__file__).parent

    parser = ArgumentParser(formatter_class=ArgumentDefaultsHelpFormatter)
    parser.add_argument("--classifier", type=str, required=True)
    args = parser.parse_args()

    cmd = ["cargo", "build", "--color=always", "--release"]

    target = classifier_to_target(args.classifier)
    if target:
        command = ["rustup", "target", "add", target]
        print("$ " + subprocess.list2cmdline(command))
        subprocess.run(command, cwd=basedir, check=True)
        cmd += ["--target", target]

    output = basedir / "target" / "bindings"
    Path(output).mkdir(exist_ok=True, parents=True)
    cmd += ["--target-dir", output]

    print("$ " + subprocess.list2cmdline(cmd))
    subprocess.run(cmd, cwd=basedir, check=True)

    artifact = get_cargo_artifact_name(args.classifier)
    src = output / target / "release" / artifact
    dst = basedir / "target" / "native" / args.classifier / artifact
    dst.parent.mkdir(exist_ok=True, parents=True)
    shutil.copy2(src, dst)
