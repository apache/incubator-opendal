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

import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# The path for current script.
SCRIPT_PATH = Path(__file__).parent.absolute()
# The path for `.github` dir.
GITHUB_DIR = SCRIPT_PATH.parent.parent
# The project dir for opendal.
PROJECT_DIR = GITHUB_DIR.parent

LANGUAGE_BINDING = ["java", "python", "ruby", "nodejs", "go", "c", "cpp", "dotnet"]

INTEGRATIONS = ["object_store"]


def provided_cases() -> list[dict[str, str]]:
    root_dir = f"{GITHUB_DIR}/services"

    cases = [
        {
            "service": service,
            "setup": setup,
            "feature": "services-{}".format(service.replace("_", "-")),
            "content": Path(
                os.path.join(root_dir, service, setup, "action.yml")
            ).read_text(),
        }
        for service in os.listdir(root_dir)
        for setup in os.listdir(os.path.join(root_dir, service))
        if os.path.exists(os.path.join(root_dir, service, setup, "action.yml"))
    ]

    # Check if this workflow needs to read secrets.
    #
    # We will check if pattern `op://services` exist in content.
    if not os.getenv("GITHUB_HAS_SECRETS") == "true":
        cases[:] = [v for v in cases if "op://services" not in v["content"]]

    # Remove content from cases.
    cases = [
        {
            "setup": v["setup"],
            "service": v["service"],
            "feature": v["feature"],
        }
        for v in cases
    ]

    # Make sure the order is stable.
    sorted_cases = sorted(cases, key=lambda x: (x["service"], x["setup"]))
    return sorted_cases


@dataclass
class Hint:
    # Is core affected?
    core: bool = field(default=False, init=False)
    # Is binding java affected?
    binding_java: bool = field(default=False, init=False)
    # Is binding python affected?
    binding_python: bool = field(default=False, init=False)
    # Is binding ruby affected?
    binding_ruby: bool = field(default=False, init=False)
    # Is binding nodejs affected?
    binding_nodejs: bool = field(default=False, init=False)
    # Is binding go affected?
    binding_go: bool = field(default=False, init=False)
    # Is binding c affected?
    binding_c: bool = field(default=False, init=False)
    # Is binding cpp affected?
    binding_cpp: bool = field(default=False, init=False)
    # Is binding dotnet affected?
    binding_dotnet: bool = field(default=False, init=False)
    # Is integration object_store affected ?
    integration_object_store: bool = field(default=False, init=False)

    # Should we run all services tests?
    all_service: bool = field(default=False, init=False)
    # affected services set.
    services: set = field(default_factory=set, init=False)


def calculate_hint(changed_files: list[str]) -> Hint:
    hint = Hint()

    # Remove all files that end with `.md`
    changed_files = [f for f in changed_files if not f.endswith(".md")]

    def mark_service_affected(service: str) -> None:
        hint.core = True
        for language in LANGUAGE_BINDING:
            setattr(hint, f"binding_{language}", True)
        for integration in INTEGRATIONS:
            setattr(hint, f"integration_{integration}", True)

        hint.services.add(service)
        hint.services.add(service.replace("-", "_"))
        hint.services.add(service.replace("_", "-"))

    for p in changed_files:
        # workflow behavior tests affected
        if p == ".github/workflows/test_behavior.yml":
            hint.core = True
            for language in LANGUAGE_BINDING:
                setattr(hint, f"binding_{language}", True)
            for integration in INTEGRATIONS:
                setattr(hint, f"integration_{integration}", True)
            hint.all_service = True

        if p in [
            ".github/workflows/test_behavior_core.yml",
            ".github/actions/test_behavior_core/action.yaml",
        ]:
            hint.core = True
            hint.all_service = True

        for language in LANGUAGE_BINDING:
            if p == f".github/workflows/test_behavior_binding_{language}.yml":
                setattr(hint, f"binding_{language}", True)
                hint.all_service = True

        for integration in INTEGRATIONS:
            if p == f".github/workflows/test_behavior_integration_{integration}.yml":
                setattr(hint, f"integration_{integration}", True)
                hint.all_service = True

        # core affected
        if (
            p.startswith("core/")
            and not p.startswith("core/benches/")
            and not p.startswith("core/edge/")
            and not p.startswith("core/fuzz/")
            and not p.startswith("core/src/services/")
            and not p.startswith("core/core/src/services/")
            and not p.startswith("core/services/")
            and not p.startswith("core/src/docs/")
            and not p.startswith("core/core/src/docs/")
        ):
            hint.core = True
            for language in LANGUAGE_BINDING:
                setattr(hint, f"binding_{language}", True)
            for integration in INTEGRATIONS:
                setattr(hint, f"integration_{integration}", True)
            hint.all_service = True

        # language binding affected
        for language in LANGUAGE_BINDING:
            if p.startswith(f"bindings/{language}/"):
                setattr(hint, f"binding_{language}", True)
                hint.all_service = True

        # c affected
        if p.startswith("bindings/c/"):
            hint.binding_c = True
            hint.binding_go = True
            hint.all_service = True

        # cpp affected
        if p.startswith("bindings/cpp/"):
            hint.binding_cpp = True
            hint.all_service = True

        # go affected
        if p.startswith(".github/scripts/test_go_binding"):
            hint.binding_go = True
            hint.all_service = True

        # integration affected
        for integration in INTEGRATIONS:
            if p.startswith(f"integrations/{integration}"):
                setattr(hint, f"integration_{integration}", True)
                hint.all_service = True

        # core service affected
        match = re.search(r"core/src/services/([^/]+)/", p)
        if match:
            mark_service_affected(match.group(1))

        # service crate affected
        match = re.search(r"core/services/([^/]+)/", p)
        if match:
            mark_service_affected(match.group(1))

        # opendal-core internal service affected
        match = re.search(r"core/core/src/services/([^/]+)/", p)
        if match:
            mark_service_affected(match.group(1))

        # core test affected
        match = re.search(r".github/services/([^/]+)/", p)
        if match:
            mark_service_affected(match.group(1))

        # fixture affected
        match = re.search(r"fixtures/([^/]+)/", p)
        if match:
            mark_service_affected(match.group(1))

    return hint


# `unique_cases` is used to only one setup for each service.
#
# We need this because we have multiple setups for each service, and they have already been
# tested by `core` workflow. So we can only test unique setup for each service for bindings.
#
# We make sure that we return the first setup for each service in alphabet order.
def unique_cases(cases):
    ucases = {}
    for case in cases:
        service = case["service"]
        if service not in ucases:
            ucases[service] = case

    # Convert the dictionary back to a list if needed
    return list(ucases.values())


def group_cases_by_service(cases: list[dict[str, str]]) -> list[dict[str, Any]]:
    grouped_cases = {}
    for case in cases:
        service = case["service"]
        if service not in grouped_cases:
            grouped_cases[service] = {
                "service": service,
                "feature": case["feature"],
                "setups": [],
            }
        grouped_cases[service]["setups"].append(case["setup"])

    return list(grouped_cases.values())


def generate_core_cases(
    cases: list[dict[str, str]], hint: Hint
) -> list[dict[str, str]]:
    # Always run all tests if it is a push event.
    if os.getenv("GITHUB_IS_PUSH") == "true":
        return cases

    # Return empty if core is False
    if not hint.core:
        return []

    # Return all services if all_service is True
    if hint.all_service:
        return cases

    # Filter all cases that not shown un in changed files
    cases = [v for v in cases if v["service"] in hint.services]
    return cases


def generate_language_binding_cases(
    cases: list[dict[str, str]], hint: Hint, language: str
) -> list[dict[str, str]]:
    cases = unique_cases(cases)

    # Disable aliyun_drive case for every language.
    #
    # This is because aliyun_drive has a speed limit and tests may not be stable enough.
    # Bindings may be treated as parallel requests, so we need to disable it for all languages.
    cases = [v for v in cases if v["service"] != "aliyun_drive"]

    # These fixtures target core services without enabling bindings.
    cases = [v for v in cases if v["service"] not in ["github", "smb"]]

    # Remove invalid cases for java.
    if language == "java":
        cases = [v for v in cases if v["service"] not in [
            "compfs",
            # FIXME: Re-enable hf after https://github.com/apache/opendal/issues/7367 is fixed.
            "hf",
            "hdfs",
            "hdfs_native",
            "monoiofs",
            "rocksdb",
        ]]

    # Remove invalid cases for dotnet.
    if language == "dotnet":
        cases = [v for v in cases if v["service"] not in [
            "hdfs",
            "hdfs_native",
            "rocksdb",
        ]]

    # Remove invalid cases for go.
    if language == "go":
        cases = [v for v in cases if v["service"] not in [
            # opendal-go-services doesn't provide TOS yet.
            "tos",
        ]]
    
    # Enable integrated services (cases) for ruby.
    # Ruby binding only integrates some services. Read more in bindings/ruby/Cargo.toml
    if language == "ruby":
        cases = [v for v in cases if v["service"] in [
            "azblob",
            "azdls",
            "cos",
            "fs",
            "gcs",
            "ghac",
            "http",
            "ipmfs",
            "memory",
            "obs",
            "oss",
            "s3",
            "webdav",
            "webhdfs",
            "azfile",
        ]]

    if os.getenv("GITHUB_IS_PUSH") == "true":
        return cases

    # Return empty if this binding is False
    if not getattr(hint, f"binding_{language}"):
        return []

    # Return all services if all_service is True
    if hint.all_service:
        return cases

    # Filter all cases that not shown up in changed files
    cases = [v for v in cases if v["service"] in hint.services]
    return cases


def generate_integration_cases(
    cases: list[dict[str, str]], hint: Hint, integration: str
) -> list[dict[str, str]]:
    # Return empty if this integration is False
    if not getattr(hint, f"integration_{integration}"):
        return []

    cases = unique_cases(cases)

    if integration == "object_store":
        supported_services = ["fs", "s3"]
        cases = [v for v in cases if v["service"] in supported_services]

    # Return all services if all_service is True
    if hint.all_service:
        return cases

    # Filter all cases that not shown up in changed files
    cases = [v for v in cases if v["service"] in hint.services]

    return cases


def plan(changed_files: list[str]) -> dict[str, Any]:
    cases = provided_cases()
    hint = calculate_hint(changed_files)

    core_cases = group_cases_by_service(generate_core_cases(cases, hint))

    jobs = {
        "components": {
            "core": False,
        },
        "core": [],
    }

    if len(core_cases) > 0:
        jobs["components"]["core"] = True
        jobs["core"].append({"os": "ubuntu-latest", "cases": core_cases})

        # fs is the only services need to run upon windows, let's hard code it here.
        if "fs" in [v["service"] for v in core_cases]:
            jobs["core"].append(
                {
                    "os": "windows-latest",
                    "cases": [
                        {
                            "setups": ["local_fs"],
                            "service": "fs",
                            "feature": "services-fs",
                        }
                    ],
                }
            )
    for language in LANGUAGE_BINDING:
        jobs[f"binding_{language}"] = []
        jobs["components"][f"binding_{language}"] = False
        language_cases = generate_language_binding_cases(cases, hint, language)
        if len(language_cases) > 0:
            jobs["components"][f"binding_{language}"] = True
            jobs[f"binding_{language}"].append(
                {"os": "ubuntu-latest", "cases": language_cases}
            )
            if language == "go":
                # Add fs service to ensure the go binding works on Windows and macOS.
                jobs[f"binding_{language}"].append(
                    {
                        "os": "windows-latest",
                        "cases": [
                            {"setup": "local_fs", "service": "fs", "feature": "services-fs"}
                        ],
                    }
                )
                jobs[f"binding_{language}"].append(
                    {
                        "os": "macos-latest",
                        "cases": [
                            {"setup": "local_fs", "service": "fs", "feature": "services-fs"}
                        ],
                    }
                )

    for integration in INTEGRATIONS:
        jobs[f"integration_{integration}"] = []
        jobs["components"][f"integration_{integration}"] = False
        integration_cases = generate_integration_cases(cases, hint, integration)
        if len(integration_cases) > 0:
            jobs["components"][f"integration_{integration}"] = True
            jobs[f"integration_{integration}"].append(
                {"os": "ubuntu-latest", "cases": integration_cases}
            )

    return jobs


if __name__ == "__main__":
    changed_files = sys.argv[1:]
    result = plan(changed_files)
    print(json.dumps(result))
