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

"""Breaking declarations, release ranges and frozen candidate inputs."""

import base64
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import release_impact as impact
import release_lifecycle as release


class ImpactTests(unittest.TestCase):
    def pull(self, body="", labeled=False):
        return {
            "body": body,
            "labels": [{"name": "breaking-changes"}] if labeled else [],
        }

    def test_compatible_pr_needs_no_declaration(self):
        for body in (
            None,
            "",
            "Fix a typo",
            "# Breaking changes\nNone",
            "# Breaking changes\nNone.\n\n# AI Usage Statement\nNone",
            "# Breaking changes\nN/A.",
            Path(".github/pull_request_template.md").read_text(),
        ):
            self.assertIsNone(
                impact.breaking_declaration(self.pull(body), impact.package_names())
            )

    def test_scoped_breaking_declaration_preserves_migration(self):
        body = "# Breaking changes\nAffected packages: core, bindings/java\n\nMigration:\n- Replace `old()` with `new()`.\n\n# AI Usage Statement\nNone"
        body = body.replace("\n", "\r\n")
        declaration = impact.breaking_declaration(
            self.pull(body, True), impact.package_names()
        )
        self.assertEqual(
            declaration,
            {
                "packages": ["bindings/java", "core"],
                "migration": "- Replace `old()` with `new()`.",
            },
        )

    def test_migration_can_contain_fenced_headings(self):
        migration = "```bash\n# Update callers\nuse_new_api\n```"
        body = f"# Breaking changes\nAffected packages: core\nMigration:\n{migration}\n# AI Usage Statement\nNone"
        self.assertEqual(
            impact.breaking_declaration(self.pull(body, True), impact.package_names())[
                "migration"
            ],
            migration,
        )

    def test_inconsistent_or_incomplete_declarations_fail(self):
        valid = (
            "# Breaking changes\nAffected packages: core\nMigration:\nUse the new API."
        )
        for body, labeled in (
            ("", True),
            ("# Breaking changes\nNone.", True),
            (valid, False),
            (valid.replace("core", "bindings/go"), True),
            (valid.replace("core", "core, core"), True),
            (valid.replace("Use the new API.", ""), True),
            (valid.replace("Use the new API.", "TODO"), True),
            (valid + "\n# Breaking changes\n", True),
        ):
            with self.subTest(body=body), self.assertRaises(ValueError):
                impact.breaking_declaration(
                    self.pull(body, labeled), impact.package_names()
                )

    def test_check_command_handles_real_event_file(self):
        with tempfile.TemporaryDirectory() as directory:
            event = Path(directory) / "event.json"
            event.write_text(json.dumps({"pull_request": self.pull()}))
            env = os.environ | {"GITHUB_EVENT_PATH": str(event)}
            subprocess.run(
                ["python3", "scripts/release_impact.py", "check"],
                env=env,
                check=True,
                capture_output=True,
            )
            event.write_text(json.dumps({"pull_request": self.pull(labeled=True)}))
            result = subprocess.run(
                ["python3", "scripts/release_impact.py", "check"],
                env=env,
                check=False,
                capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)

    def test_collection_uses_exact_merged_main_commit(self):
        sha = "a" * 40
        pull = self.pull(
            "# Breaking changes\nAffected packages: core\nMigration:\nUse the replacement.",
            True,
        ) | {
            "number": 1,
            "html_url": "https://github.com/apache/opendal/pull/1",
            "merged_at": "now",
            "merge_commit_sha": sha,
            "base": {"ref": "main", "repo": {"full_name": release.REPO}},
        }
        unrelated = pull | {"merge_commit_sha": "b" * 40}
        with (
            patch.object(impact, "source_commits", return_value=(sha, [sha])),
            patch.object(release, "pages", return_value=[unrelated, pull]),
        ):
            plan = impact.collect_plan("v0.59.1", sha, impact.package_names())
            self.assertEqual(plan["pulls"][0]["breaking"]["packages"], ["core"])
        with (
            patch.object(impact, "source_commits", return_value=(sha, [sha])),
            patch.object(release, "pages", return_value=[]),
            self.assertRaisesRegex(ValueError, "expected one merged main PR"),
        ):
            impact.collect_plan("v0.59.1", sha, impact.package_names())

    def test_release_range_starts_at_published_source_despite_squashed_sync(self):
        with tempfile.TemporaryDirectory() as directory:

            def git(*args):
                return subprocess.check_output(
                    ["git", *args], cwd=directory, text=True
                ).strip()

            git("init", "-q", "-b", "main")
            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.invalid")

            def commit(message):
                git("add", ".")
                git(
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "--allow-empty",
                    "-qm",
                    message,
                )
                return git("rev-parse", "HEAD")

            base = commit("Released source")
            git("checkout", "-qb", "candidate")
            commit("Mechanical preparation")
            git("-c", "tag.gpgsign=false", "tag", "v0.59.1")
            plan_path = Path(directory) / impact.PLAN_PATH
            plan_path.parent.mkdir()
            plan_path.write_text(json.dumps({"source": base}))
            commit("Candidate with a frozen plan")
            git("-c", "tag.gpgsign=false", "tag", "v0.59.2")
            git("checkout", "-q", "main")
            fix = commit("Next PR")
            sync = commit("Squashed version sync")
            with patch.object(
                release, "command", side_effect=lambda *args: git(*args[1:])
            ):
                for baseline in ("v0.59.1", "v0.59.2"):
                    self.assertEqual(
                        impact.source_commits(baseline, sync), (sync, [fix, sync])
                    )

    def test_prepare_passes_scoped_breaking_and_reuses_frozen_declarations(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plan.json"
            sha = "a" * 40
            plan = {
                "baseline": "v0.59.1",
                "source": sha,
                "pulls": [
                    {
                        "number": 1,
                        "url": "url",
                        "breaking": {
                            "packages": ["core"],
                            "migration": "Use replacement",
                        },
                    },
                    {
                        "number": 2,
                        "url": "url",
                        "breaking": {
                            "packages": ["core"],
                            "migration": "Update callers",
                        },
                    },
                ],
            }
            path.write_text(json.dumps(plan))

            def run(args, **kwargs):
                self.assertEqual(args.count("--breaking"), 1)
                self.assertEqual(args[-2:], ["--breaking", "core"])
                Path(args[args.index("--report") + 1]).write_text("[]")

            with (
                patch.object(impact, "PLAN_PATH", str(path)),
                patch.object(release, "command", return_value=sha),
                patch.object(impact, "collect_plan") as collect,
                patch.object(impact.subprocess, "run", side_effect=run),
            ):
                impact.prepare("v0.59.1", sha)
                collect.assert_not_called()
            self.assertEqual(json.loads(path.read_text())["pulls"], plan["pulls"])

    def test_notice_reads_migration_from_candidate_not_live_pr(self):
        candidate = release.Candidate("0.60.0-rc.1", "a" * 40)
        plan = {
            "versions": [
                {
                    "package": "core",
                    "previous": "0.59.1",
                    "configured": "0.59.1",
                    "target": "0.60.0",
                    "breaking": True,
                    "public_dependency_reason": None,
                }
            ],
            "pulls": [
                {
                    "number": 12,
                    "url": "https://github.com/apache/opendal/pull/12",
                    "breaking": {
                        "packages": ["core"],
                        "migration": "Use `new()` instead.",
                    },
                }
            ],
        }
        content = {"content": base64.b64encode(json.dumps(plan).encode()).decode()}
        with patch.object(release, "api", return_value=content) as api:
            notes = release.version_notes(candidate)
            self.assertIn("0.60.0", notes)
            self.assertIn("Use `new()` instead.", notes)
            self.assertIn(candidate.sha, api.call_args.args[0])
        with (
            patch.object(release, "version_notes", return_value=notes),
            patch.object(
                release, "discussion", return_value={"id": "discussion"}
            ) as discussion,
            patch.object(release, "comment_once"),
        ):
            release.notice(
                candidate,
                {"phase": "release_candidate_draft", "latest_revision_number": "00001"},
            )
            self.assertIn(notes, discussion.call_args.args[1])


if __name__ == "__main__":
    unittest.main()
