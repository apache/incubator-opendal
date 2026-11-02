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

"""Rebuild every source archive in two checkouts with different filesystem metadata."""

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tempfile
from pathlib import Path


def run(*args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--revision", default="HEAD")
    parser.add_argument(
        "--working-tree",
        action="store_true",
        help="Include tracked local changes in both checkouts",
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--compare", type=Path, help="Also compare with downloaded CI tar.gz artifacts"
    )
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    sha = subprocess.check_output(
        ["git", "rev-parse", args.revision], cwd=repo, text=True
    ).strip()
    patch = (
        subprocess.check_output(["git", "diff", "--binary", sha], cwd=repo)
        if args.working_tree
        else b""
    )
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / "input.patch").write_bytes(patch)
    report = {
        "platform": platform.platform(),
        "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
        "commit": sha,
        "patch_sha512": hashlib.sha512(patch).hexdigest(),
        "runs": [],
        "matched": False,
    }
    try:
        with tempfile.TemporaryDirectory(prefix="opendal-reproduce-") as tmp:
            target = Path(tmp) / "target"
            for index in range(2):
                checkout = Path(tmp) / f"checkout-{index}"
                run(
                    "git",
                    "clone",
                    "--shared",
                    "--no-checkout",
                    "-c",
                    "core.autocrlf=false",
                    repo,
                    checkout,
                )
                run("git", "checkout", "--detach", sha, cwd=checkout)
                if patch:
                    run("git", "apply", "--binary", "-", cwd=checkout, input=patch)
                tracked = subprocess.check_output(
                    ["git", "ls-files", "-z"], cwd=checkout
                ).split(b"\0")
                for raw in filter(None, tracked):
                    file = checkout / os.fsdecode(raw)
                    if file.is_symlink() or not file.is_file():
                        continue
                    os.utime(file, (1_700_000_000 + index * 100_000_000,) * 2)
                    if os.name == "posix":
                        os.chmod(file, 0o644 if index == 0 else 0o755)
                env = {
                    **os.environ,
                    "CARGO_TARGET_DIR": str(target),
                    "TZ": "UTC" if index == 0 else "Asia/Shanghai",
                }
                log_path = args.output / f"build-{index}.log"
                with log_path.open("wb") as log:
                    run(
                        "cargo",
                        "run",
                        "--locked",
                        "--manifest-path",
                        checkout / "dev/Cargo.toml",
                        "--",
                        "release",
                        "--unsigned",
                        cwd=checkout,
                        env=env,
                        stdout=log,
                        stderr=subprocess.STDOUT,
                    )
                inventory = json.loads(
                    subprocess.check_output(
                        [str(target / "debug/odev"), "release-packages"], cwd=checkout
                    )
                )
                archives = sorted((checkout / "dist").glob("*.tar.gz"))
                if len(archives) != len(inventory) or not archives:
                    raise ValueError("source archive inventory is incomplete")
                hashes = {
                    file.name: hashlib.sha512(file.read_bytes()).hexdigest()
                    for file in archives
                }
                report["runs"].append(hashes)
                if index == 0:
                    for file in archives:
                        shutil.copy2(file, args.output / file.name)
                        shutil.copy2(
                            Path(str(file) + ".sha512"),
                            args.output / (file.name + ".sha512"),
                        )
                if index and hashes != report["runs"][0]:
                    raise ValueError("independent checkout artifacts differ")
            if args.compare:
                expected = {
                    file.name: hashlib.sha512(file.read_bytes()).hexdigest()
                    for file in args.compare.glob("*.tar.gz")
                }
                if expected != report["runs"][0]:
                    raise ValueError(
                        "downloaded CI artifacts differ or inventory does not match"
                    )
            report["matched"] = True
    finally:
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(
        f"Verified {len(report['runs'][0])} identical source archives. Evidence: {args.output}"
    )


if __name__ == "__main__":
    main()
