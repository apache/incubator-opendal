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

from pathlib import Path

ROOT_DIR = Path(__file__).parent.parent


def list_packages() -> list[Path]:
    packages = [Path("core")]

    for dir in ["integrations", "bin", "bindings"]:
        base = ROOT_DIR / dir
        if not base.exists():
            # Skip directories that do not exist in this workspace
            continue
        cur = []
        for path in base.iterdir():
            if path.is_dir():
                cur.append(path.relative_to(ROOT_DIR))
        cur.sort()
        packages.extend(cur)
    return packages


PACKAGES = list_packages()

PACKAGE_FEATURES: dict[Path, list[str]] = {
    Path("bindings/nodejs"): ["services-all"],
    Path("bindings/python"): ["services-all"],
}
