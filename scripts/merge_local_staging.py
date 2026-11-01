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

import sys
from pathlib import Path
import shutil


# Copy the contents of the source directory to the target directory,
# appending the contents of the .index file
def copy_and_append_index(target, source):
    for src_dir in source.iterdir():
        if src_dir.is_dir():
            dst_dir = target / src_dir.name
            dst_dir.mkdir(parents=True, exist_ok=True)

            src_path = src_dir / ".index"
            dst_path = dst_dir / ".index"
            if src_path.exists():
                with src_path.open("r") as src, dst_path.open("a") as dst:
                    print(f"Appending {src_path} to {dst_path}")
                    dst.write(src.read())

            for item in src_dir.iterdir():
                if item.name != ".index":  # Avoid copying the .index file twice
                    print(f"Copying {item} to {dst_dir}")
                    if item.is_dir():
                        shutil.copytree(item, dst_dir / item.name, dirs_exist_ok=True)
                    else:
                        shutil.copy2(item, dst_dir / item.name)


def print_directory_contents(directory, prefix=""):
    for item in directory.iterdir():
        print(f"{prefix}{item.relative_to(directory)}")
        if item.is_dir():
            print_directory_contents(item, prefix + "    ")


def print_index_contents(directory):
    for sub_dir in directory.rglob(".index"):
        with sub_dir.open("r") as file:
            print(file.read())


def main():
    if len(sys.argv) < 3:
        print("Usage: merge_local_staging.py <target> <source> [<source> ...]")
        sys.exit(1)

    target = Path(sys.argv[1])
    print(f"Target directory set to {target}")

    for dir_path in sys.argv[2:]:
        source = Path(dir_path)
        if source.is_dir():
            print(f"Processing {source}")
            copy_and_append_index(target, source)
        else:
            print(f"{dir_path} is not a valid directory.")
            sys.exit(1)

    # Print content of target for debugging.
    print(f"Content of {target}:")
    print_directory_contents(target)
    print("Content of index:")
    print_index_contents(target)


if __name__ == "__main__":
    main()
