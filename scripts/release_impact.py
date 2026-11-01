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

"""Validate optional breaking declarations and freeze weekly version decisions."""

import argparse
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path

PLAN_PATH = ".release/plan.json"


def package_names():
    inventory = Path("dev/src/release/package.rs").read_text()
    return set(re.findall(r'make_package\("([^"\n]+)", "[0-9]', inventory))


def breaking_declaration(pull, packages):
    body = re.sub(
        r"<!--.*?-->",
        "",
        (pull.get("body") or "").replace("\r\n", "\n"),
        flags=re.DOTALL,
    )
    # Headings in fenced migration examples are content, not section boundaries.
    heading_text = re.sub(
        r"(?ms)^(`{3,}|~{3,})[^\n]*\n.*?^\1[ \t]*$",
        lambda match: "".join("\n" if c == "\n" else " " for c in match[0]),
        body,
    )
    headings = list(re.finditer(r"^(#{1,6}) +(.+?)[ \t]*$", heading_text, re.MULTILINE))
    sections = []
    for index, heading in enumerate(headings):
        if heading[2].strip().lower() != "breaking changes":
            continue
        end = next(
            (h.start() for h in headings[index + 1 :] if len(h[1]) <= len(heading[1])),
            len(body),
        )
        sections.append(body[heading.end() : end].strip())
    if len(sections) > 1:
        raise ValueError("keep only one Breaking changes section")
    section = sections[0] if sections else ""
    labeled = any(
        label["name"] == "breaking-changes" for label in pull.get("labels", [])
    )
    if section.lower().rstrip(".").strip() in {
        "",
        "none",
        "n/a",
        "no breaking changes",
    }:
        if labeled:
            raise ValueError(
                "breaking-changes requires affected packages and migration instructions"
            )
        return None
    if not labeled:
        raise ValueError(
            "add the breaking-changes label or leave Breaking changes empty"
        )
    match = re.fullmatch(
        r"Affected packages: *([^\n]+)\n+Migration: *\n?([\s\S]+)",
        section,
        re.IGNORECASE,
    )
    if not match:
        raise ValueError(
            "expected Affected packages: <comma-separated names> followed by Migration: <instructions>"
        )
    affected = [name.strip().strip("`") for name in match[1].split(",")]
    if not affected or len(affected) != len(set(affected)) or set(affected) - packages:
        raise ValueError(
            f"invalid affected packages: {match[1]}; allowed: {', '.join(sorted(packages))}"
        )
    migration = match[2].strip()
    if not re.search(r"[a-zA-Z0-9]", migration) or migration.lower().strip(" .-") in {
        "none",
        "n/a",
        "todo",
        "tbd",
    }:
        raise ValueError("provide migration instructions for the breaking change")
    return {"packages": sorted(affected), "migration": migration}


def source_commits(baseline, source):
    from release_lifecycle import command

    source = command("git", "rev-parse", f"{source}^{{commit}}")
    exists = command("git", "ls-tree", "--name-only", baseline, PLAN_PATH)
    if exists:
        previous = json.loads(command("git", "show", f"{baseline}:{PLAN_PATH}"))[
            "source"
        ]
    else:
        # Older candidates have a mechanical release commit off main, without a plan.
        previous = command("git", "merge-base", baseline, source)
    command("git", "merge-base", "--is-ancestor", previous, source)
    commits = command(
        "git", "rev-list", "--first-parent", "--reverse", f"{previous}..{source}"
    ).splitlines()
    return source, commits


def collect_plan(baseline, source, packages):
    from release_lifecycle import REPO, pages

    source, commits = source_commits(baseline, source)
    pulls = []
    for sha in commits:
        matches = [
            pull
            for pull in pages(f"repos/{REPO}/commits/{sha}/pulls?per_page=100")
            if pull.get("merged_at")
            and pull["merge_commit_sha"] == sha
            and pull["base"]["ref"] == "main"
            and pull["base"]["repo"]["full_name"] == REPO
        ]
        if len(matches) != 1:
            raise ValueError(
                f"expected one merged main PR for {sha}; found {len(matches)}"
            )
        pull = matches[0]
        try:
            declaration = breaking_declaration(pull, packages)
        except ValueError as error:
            raise ValueError(f"PR #{pull['number']}: {error}") from error
        pulls.append(
            {
                "number": pull["number"],
                "url": pull["html_url"],
                "commit": sha,
                "breaking": declaration,
            }
        )
    return {"baseline": baseline, "source": source, "pulls": pulls}


def prepare(baseline, source):
    from release_lifecycle import command

    path = Path(PLAN_PATH)
    source = command("git", "rev-parse", f"{source}^{{commit}}")
    if path.exists():
        plan = json.loads(path.read_text())
        if (plan["baseline"], plan["source"]) != (baseline, source):
            plan = collect_plan(baseline, source, package_names())
    else:
        plan = collect_plan(baseline, source, package_names())
    breaking = sorted(
        {
            package
            for pull in plan["pulls"]
            if pull["breaking"]
            for package in pull["breaking"]["packages"]
        }
    )
    with tempfile.TemporaryDirectory(prefix="opendal-release-plan-") as directory:
        report = Path(directory) / "versions.json"
        args = [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            "dev/Cargo.toml",
            "--",
            "update-version",
            "--baseline",
            baseline,
            "--patch",
            "--report",
            str(report),
        ]
        for package in breaking:
            args += ["--breaking", package]
        subprocess.run(args, check=True)
        versions = json.loads(report.read_text())
        if "versions" in plan:
            frozen = {v["package"]: v["target"] for v in plan["versions"]}
            if frozen != {v["package"]: v["target"] for v in versions}:
                raise ValueError(
                    "version targets differ from the existing plan; prepare from the fixed source commit"
                )
        else:
            plan["versions"] = versions
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(plan, indent=2) + "\n")
    print(render_plan(plan))


def render_plan(plan):
    lines = [
        "### Package versions",
        "",
        "| Package | Previous | Target | Reason |",
        "| --- | --- | --- | --- |",
    ]
    for version in plan["versions"]:
        reasons = []
        if version["breaking"]:
            reasons.append("Declared breaking change")
        if version["public_dependency_reason"]:
            reasons.append("Public dependency compatibility")
        if version["target"] == version["configured"]:
            reasons.append("Configured version retained")
        if not reasons:
            reasons.append("Weekly patch")
        lines.append(
            f"| {version['package']} | {version['previous'] or 'New package'} | {version['target']} | {'; '.join(reasons)} |"
        )
    lines += ["", "### Breaking changes and migration", ""]
    for pull in plan["pulls"]:
        if pull["breaking"]:
            names = ", ".join(f"`{name}`" for name in pull["breaking"]["packages"])
            lines += [
                f"#### [#{pull['number']}]({pull['url']}) — {names}",
                "",
                pull["breaking"]["migration"],
                "",
            ]
    if not any(pull["breaking"] for pull in plan["pulls"]):
        lines.append("No breaking changes declared in this release range.")
    for version in plan["versions"]:
        if version["public_dependency_reason"]:
            lines += [
                "",
                f"Public dependency adjustment for `{version['package']}`:",
                "",
                "```text",
                version["public_dependency_reason"],
                "```",
            ]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="action", required=True)
    subparsers.add_parser("check")
    prepare_parser = subparsers.add_parser("prepare")
    prepare_parser.add_argument("--baseline", required=True)
    prepare_parser.add_argument("--source", required=True)
    args = parser.parse_args()
    if args.action == "check":
        pull = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())[
            "pull_request"
        ]
        breaking_declaration(pull, package_names())
        print("Breaking change declaration is consistent.")
    else:
        prepare(args.baseline, args.source)


if __name__ == "__main__":
    main()
