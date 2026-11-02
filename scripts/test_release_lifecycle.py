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

"""Release lifecycle contracts at GitHub/ATR and Git boundaries."""

import json
import os
import subprocess
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import patch

import release_lifecycle as release


class LifecycleTests(unittest.TestCase):
    def setUp(self):
        self.candidate = release.Candidate("0.59.3-rc.2", "a" * 40)
        self.atr = {
            "phase": "release_preview",
            "vote_resolved": "2026-09-14T00:00:00",
            "latest_revision_number": "00003",
            "current_vote_seq": 1,
        }

    def test_rc_branch_and_tag_must_agree(self):
        with (
            patch.object(release, "ref_sha", side_effect=["a" * 40, "b" * 40]),
            self.assertRaisesRegex(ValueError, "disagree"),
        ):
            release.Candidate.load("0.59.3-rc.2")
        for rc in ("0.59.3", "0.59.3-rc.0", "../main"):
            with self.subTest(rc=rc), self.assertRaises(ValueError):
                release.Candidate(rc, "a" * 40)

    def test_discovery_excludes_legacy_candidates_and_final_branches(self):
        refs = [
            {"ref": "refs/heads/release-candidates/weekly-34560096534-1"},
            {"ref": "refs/heads/releases/0.59.3"},
            {"ref": "refs/heads/releases/0.59.3-rc.1"},
        ]
        with patch.object(release, "pages", return_value=refs) as pages:
            self.assertEqual(release.candidates(), ["0.59.3-rc.1"])
            pages.assert_called_once_with(
                "repos/apache/opendal/git/matching-refs/heads/releases/"
            )

    def test_only_resolved_passed_votes_can_publish(self):
        for phase, resolved, expected in (
            ("release_candidate_draft", None, False),
            ("release_candidate", "now", False),
            ("release_preview", None, False),
            ("release_preview", "now", True),
            ("release", "now", True),
        ):
            with self.subTest(phase=phase, resolved=resolved):
                self.assertEqual(
                    release.passed({"phase": phase, "vote_resolved": resolved}),
                    expected,
                )
        with (
            patch.object(
                release.Candidate, "atr", return_value={"phase": "release_candidate"}
            ),
            patch.object(release, "final_refs") as refs,
        ):
            with self.assertRaisesRegex(ValueError, "not resolved"):
                release.publish(self.candidate)
            refs.assert_not_called()

    def test_conflicting_final_ref_cannot_be_replaced(self):
        with (
            patch.object(release, "ref_sha", side_effect=["b" * 40, None]),
            patch.object(release, "command") as command,
        ):
            with self.assertRaisesRegex(ValueError, "another commit"):
                release.final_refs(self.candidate)
            command.assert_not_called()
        with (
            patch.object(release, "ref_sha", return_value="a" * 40),
            patch.object(release, "command") as command,
        ):
            release.final_refs(self.candidate)
            command.assert_not_called()

    def test_partial_dispatch_resumes_only_missing_workflows(self):
        done = {
            "headSha": "a" * 40,
            "event": "workflow_dispatch",
            "status": "completed",
            "conclusion": "success",
        }
        with (
            patch.object(release, "api", return_value={"state": "active"}),
            patch.object(
                release,
                "workflow_runs",
                side_effect=lambda w, ref: [] if w == "release_python.yml" else [done],
            ),
            patch.object(release, "dispatch") as dispatch,
        ):
            self.assertFalse(release.publish_builds(self.candidate))
            dispatch.assert_called_once_with("release_python.yml", "v0.59.3", {})
        with (
            patch.object(release, "api", return_value={"state": "disabled_manually"}),
            patch.object(release, "dispatch") as dispatch,
        ):
            self.assertTrue(release.publish_builds(self.candidate))
            dispatch.assert_not_called()

    def test_failed_publication_requires_existing_run_recovery(self):
        failed = {
            "headSha": "a" * 40,
            "event": "workflow_dispatch",
            "status": "completed",
            "conclusion": "failure",
            "url": "https://example.invalid/run",
        }
        with (
            patch.object(release, "api", return_value={"state": "active"}),
            patch.object(release, "workflow_runs", return_value=[failed]),
            patch.object(release, "dispatch") as dispatch,
        ):
            with self.assertRaisesRegex(RuntimeError, "Rerun failed"):
                release.publish_builds(self.candidate)
            dispatch.assert_not_called()

    def test_optional_distribution_does_not_block_source_publication(self):
        for outcome in (False, RuntimeError("Dart publication failed")):
            with self.subTest(outcome=outcome):

                def distribute(candidate, outcome=outcome):
                    created = [
                        call.args[1]
                        for call in github.call_args_list
                        if call.args[0] == "repos/apache/opendal/releases"
                    ]
                    self.assertEqual(len(created), 1)
                    self.assertEqual(created[0]["name"], "v0.59.3")
                    self.assertEqual(created[0]["tag_name"], "v0.59.3")
                    self.assertTrue(created[0]["generate_release_notes"])
                    self.assertEqual(
                        http.call_args.args[0],
                        f"{release.ATR}/api/publisher/release/announce",
                    )
                    sync_pr.assert_called_once()
                    if isinstance(outcome, Exception):
                        raise outcome
                    return outcome

                with (
                    patch.object(
                        release.Candidate,
                        "atr",
                        side_effect=[self.atr, self.atr | {"phase": "release"}],
                    ),
                    patch.object(release, "final_refs"),
                    patch.object(release, "publish_builds", side_effect=distribute),
                    patch.object(release, "nexus_release", return_value=True) as nexus,
                    patch.object(release, "api", return_value=None) as github,
                    patch.object(release, "version_notes", return_value=""),
                    patch.object(
                        release, "request_json", side_effect=[{"value": "jwt"}, {}]
                    ) as http,
                    patch.object(release, "discussion"),
                    patch.object(
                        release, "sync_versions", return_value="sync-pr"
                    ) as sync_pr,
                    patch.object(
                        release, "notice", return_value={"id": "discussion"}
                    ) as notice,
                    patch.object(release, "comment_once"),
                    patch.object(release, "cleanup") as cleanup,
                    patch.dict(
                        os.environ,
                        {
                            "ACTIONS_ID_TOKEN_REQUEST_URL": "https://example.invalid/token?request=1",
                            "ACTIONS_ID_TOKEN_REQUEST_TOKEN": "test",
                        },
                    ),
                ):
                    release.publish(self.candidate)
                    nexus.assert_called_once_with(self.candidate)
                    cleanup.assert_not_called()
                    self.assertIn("has been released", notice.call_args.args[2])
                    self.assertIn(
                        "Optional package distribution", notice.call_args.args[2]
                    )

    def test_announcement_resume_uses_external_completion_records(self):
        atr = self.atr | {"phase": "release", "latest_revision_number": None}
        with (
            patch.object(release.Candidate, "atr", return_value=atr),
            patch.object(release, "final_refs"),
            patch.object(release, "publish_builds", return_value=True),
            patch.object(release, "nexus_release", return_value=True),
            patch.object(
                release, "api", return_value={"draft": False, "prerelease": False}
            ) as api,
            patch.object(release, "request_json") as http,
            patch.object(
                release, "discussion", return_value={"id": "discussion"}
            ) as discussion,
            patch.object(
                release, "sync_versions", return_value="https://example.invalid/pr"
            ),
            patch.object(release, "comment_once"),
            patch.object(release, "cleanup") as cleanup,
        ):
            release.publish(self.candidate)
            http.assert_not_called()  # ATR has already sent the email.
            self.assertEqual(api.call_count, 1)  # GitHub Release already exists.
            cleanup.assert_called_once_with(self.candidate)
            notice_body = discussion.call_args.args[1]
            self.assertIn("has been released", notice_body)
            self.assertNotIn("VoteStartArgs", notice_body)
            self.assertNotIn("None", notice_body)

    def test_unpublished_notice_requires_revision(self):
        for phase in (
            "release_candidate_draft",
            "release_candidate",
            "release_preview",
        ):
            with (
                self.subTest(phase=phase),
                self.assertRaisesRegex(ValueError, "revision"),
            ):
                release.notice(
                    self.candidate,
                    self.atr | {"phase": phase, "latest_revision_number": None},
                )

    def test_sync_pr_can_recover_after_branch_push(self):
        with (
            patch.object(release, "gh", return_value="[]"),
            patch.object(release, "ref_sha", return_value="a" * 40),
            patch.object(
                release, "command", return_value="https://example.invalid/pr"
            ) as command,
        ):
            self.assertEqual(
                release.sync_versions(self.candidate), "https://example.invalid/pr"
            )
            self.assertEqual(command.call_args.args[:3], ("gh", "pr", "create"))
            self.assertEqual(command.call_count, 1)

    def test_sync_outputs_approved_candidates_even_if_another_candidate_fails(self):
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.dict(os.environ, {"GITHUB_OUTPUT": f"{directory}/outputs"}),
            patch.object(
                release, "candidates", return_value=["bad", self.candidate.rc]
            ),
            patch.object(
                release.Candidate,
                "load",
                side_effect=[ValueError("bad ref"), self.candidate],
            ),
            patch.object(release.Candidate, "atr", return_value=self.atr),
            patch.object(release, "notice"),
            patch.object(release, "dispatch") as dispatch,
        ):
            with self.assertRaisesRegex(RuntimeError, "bad ref"):
                release.sync()
            self.assertEqual(
                Path(directory, "outputs").read_text(),
                'candidates=["0.59.3-rc.2"]\n',
            )
            dispatch.assert_not_called()

    def test_nexus_promotes_the_closed_rc_repository_and_waits_for_central(self):
        pom = '<project xmlns="http://maven.apache.org/POM/4.0.0"><version>0.49.2</version></project>'
        run = {"headSha": self.candidate.sha, "conclusion": "success", "databaseId": 42}
        for state, expected_posts in (("closed", 2), ("released", 1)):
            with (
                self.subTest(state=state),
                patch.object(release, "api", return_value={"state": "active"}),
                patch.object(release, "command", return_value=pom),
                patch.object(
                    release.urllib.request,
                    "urlopen",
                    side_effect=urllib.error.HTTPError(
                        "central", 404, "missing", {}, None
                    ),
                ),
                patch.object(release, "workflow_runs", return_value=[run]),
                patch.object(
                    release,
                    "gh",
                    return_value="deployByRepositoryId/orgapacheopendal-1090/",
                ),
                patch.object(
                    release,
                    "request_json",
                    side_effect=[
                        {"type": state, "transitioning": False},
                        None,
                    ],
                ) as http,
                patch.dict(
                    os.environ, {"NEXUS_USER": "test", "NEXUS_PASSWORD": "test"}
                ),
            ):
                self.assertFalse(release.nexus_release(self.candidate))
                self.assertEqual(http.call_count, expected_posts)
                if state == "closed":
                    self.assertEqual(
                        http.call_args.args[1]["data"]["stagedRepositoryIds"],
                        ["orgapacheopendal-1090"],
                    )

    def test_new_announcement_uses_approved_atr_revision_and_waits_for_confirmation(
        self,
    ):
        with (
            patch.object(
                release.Candidate,
                "atr",
                side_effect=[self.atr, self.atr | {"phase": "release"}],
            ),
            patch.object(release, "final_refs"),
            patch.object(release, "publish_builds", return_value=True),
            patch.object(release, "nexus_release", return_value=True),
            patch.object(release, "api", return_value=None) as github,
            patch.object(
                release, "version_notes", return_value="Frozen migration instructions"
            ),
            patch.object(
                release, "request_json", side_effect=[{"value": "test-jwt"}, {}]
            ) as http,
            patch.object(release, "discussion"),
            patch.object(
                release, "sync_versions", return_value="https://example.invalid/pr"
            ),
            patch.object(release, "notice", return_value={"id": "discussion"}),
            patch.object(release, "comment_once"),
            patch.object(release, "cleanup") as cleanup,
            patch.dict(
                os.environ,
                {
                    "ACTIONS_ID_TOKEN_REQUEST_URL": "https://example.invalid/token?request=1",
                    "ACTIONS_ID_TOKEN_REQUEST_TOKEN": "test",
                },
            ),
        ):
            release.publish(self.candidate)
            self.assertEqual(
                http.call_args.args[0], f"{release.ATR}/api/publisher/release/announce"
            )
            self.assertIn(
                "Frozen migration instructions", github.call_args.args[1]["body"]
            )
            payload = http.call_args.args[1]
            self.assertEqual(payload["version"], self.candidate.rc)
            self.assertEqual(payload["revision"], "00003")
            self.assertEqual(payload["email_to"], "announce@apache.org")
            self.assertNotIn("commit_hash", payload)
            cleanup.assert_called_once()

    def test_vote_reminder_deduplication_survives_restart(self):
        marker = "<!-- opendal-vote:0.59.3-rc.2:1 -->"
        data = [{"data": {"node": {"comments": {"nodes": [{"body": marker}]}}}}]
        with (
            patch.object(release, "gh", return_value=json.dumps(data)),
            patch.object(release, "api") as api,
        ):
            release.comment_once("discussion", marker, "Vote now")
            api.assert_not_called()

    def test_sync_announces_vote_without_editing_existing_discussion(self):
        discussion = {
            "id": "discussion",
            "title": f"Release candidate: {self.candidate.rc}",
            "body": "Candidate uploaded; RM verification is next.",
        }
        comments = []

        def github_read(*args):
            return json.dumps([{"data": {"node": {"comments": {"nodes": comments}}}}])

        def github_write(path, payload):
            self.assertIn("addDiscussionComment(", payload["query"])
            comments.append({"body": payload["variables"]["body"]})

        with (
            tempfile.TemporaryDirectory() as directory,
            patch.dict(os.environ, {"GITHUB_OUTPUT": f"{directory}/outputs"}),
            patch.object(release, "candidates", return_value=[self.candidate.rc]),
            patch.object(release.Candidate, "load", return_value=self.candidate),
            patch.object(
                release.Candidate,
                "atr",
                return_value=self.atr
                | {"phase": "release_candidate", "vote_resolved": None},
            ),
            patch.object(release, "version_notes", return_value="Package versions"),
            patch.object(
                release, "discussion_data", return_value=("repo", {}, [discussion])
            ),
            patch.object(release, "gh", side_effect=github_read),
            patch.object(release, "api", side_effect=github_write),
        ):
            release.sync()
            release.sync()
        self.assertEqual(len(comments), 1)
        self.assertIn("Voting is now open", comments[0]["body"])
        self.assertEqual(
            discussion["body"], "Candidate uploaded; RM verification is next."
        )

    def test_publication_status_comments_only_change_with_progress(self):
        comments = []

        def github_write(path, payload):
            self.assertIn("addDiscussionComment(", payload["query"])
            comments.append({"body": payload["variables"]["body"]})

        with (
            patch.object(release, "version_notes", return_value="Package versions"),
            patch.object(release, "discussion", return_value={"id": "discussion"}),
            patch.object(
                release,
                "gh",
                side_effect=lambda *args: json.dumps(
                    [{"data": {"node": {"comments": {"nodes": comments}}}}]
                ),
            ),
            patch.object(release, "api", side_effect=github_write),
        ):
            release.notice(self.candidate, self.atr, "Publication is running.")
            release.notice(self.candidate, self.atr, "Publication is running.")
            release.notice(
                self.candidate,
                self.atr | {"phase": "release"},
                "Publication is complete.",
            )
            release.notice(
                self.candidate,
                self.atr | {"phase": "release"},
                "Publication is complete.",
            )
        self.assertEqual(len(comments), 2)
        self.assertIn("Publication is running.", comments[0]["body"])
        self.assertIn("Publication is complete.", comments[1]["body"])
        self.assertIn("tree/releases/0.59.3)", comments[1]["body"])

    def test_baseline_does_not_require_git_ancestry(self):
        releases = [
            {
                "tag_name": "v0.59.3",
                "draft": False,
                "prerelease": False,
                "published_at": "2026-09-14",
            },
            {
                "tag_name": "v0.59.4",
                "draft": True,
                "prerelease": False,
                "published_at": "2026-09-15",
            },
            {
                "tag_name": "v0.59.4-rc.1",
                "draft": False,
                "prerelease": True,
                "published_at": "2026-09-15",
            },
        ]
        with (
            patch.object(release, "pages", return_value=releases),
            patch("sys.argv", ["release_lifecycle.py", "baseline"]),
            patch("builtins.print") as output,
        ):
            release.main()
            output.assert_called_once_with("v0.59.3")

    def test_cleanup_is_scoped_and_preserves_tags(self):
        with (
            patch.object(release, "ref_sha", return_value="a" * 40),
            patch.object(
                release, "candidates", return_value=["0.59.3-rc.1", "0.59.4-rc.1"]
            ),
            patch.object(
                release.Candidate,
                "load",
                return_value=release.Candidate("0.59.3-rc.1", "b" * 40),
            ),
            patch.object(release, "command") as command,
        ):
            release.cleanup(self.candidate)
            command.assert_called_once()
            args = command.call_args.args
            self.assertIn(":refs/heads/releases/0.59.3-rc.1", args)
            self.assertIn(
                "--force-with-lease=refs/heads/releases/0.59.3-rc.1:" + "b" * 40, args
            )
            self.assertFalse(any(":refs/tags/" in arg for arg in args))

    def test_cleanup_keeps_approved_branch_until_other_deletions_succeed(self):
        with (
            patch.object(release, "ref_sha", return_value=self.candidate.sha),
            patch.object(
                release, "candidates", return_value=[self.candidate.rc, "0.59.3-rc.1"]
            ),
            patch.object(
                release.Candidate,
                "load",
                side_effect=lambda rc: release.Candidate(rc, "a" * 40),
            ),
            patch.object(
                release, "command", side_effect=RuntimeError("branch changed")
            ) as command,
        ):
            with self.assertRaisesRegex(RuntimeError, "branch changed"):
                release.cleanup(self.candidate)
            command.assert_called_once()
            self.assertEqual(
                command.call_args.args[-1], ":refs/heads/releases/0.59.3-rc.1"
            )

    def test_real_git_signed_final_refs(self):
        previous = Path.cwd()
        with tempfile.TemporaryDirectory(prefix="od-ref-test-", dir="/tmp") as tmp:
            root = Path(tmp)
            remote, checkout, keys = (
                root / "remote.git",
                root / "checkout",
                root / "keys",
            )
            keys.mkdir(mode=0o700)
            release.command("git", "init", "--bare", str(remote))
            release.command("git", "clone", str(remote), str(checkout))
            try:
                os.chdir(checkout)
                release.command(
                    "git",
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "Candidate",
                )
                sha = release.command("git", "rev-parse", "HEAD")
                release.command(
                    "git", "push", "origin", "HEAD:refs/heads/releases/0.59.3-rc.2"
                )
                release.command(
                    "gpg",
                    "--homedir",
                    str(keys),
                    "--batch",
                    "--pinentry-mode",
                    "loopback",
                    "--passphrase",
                    "",
                    "--quick-generate-key",
                    "Test <test@example.invalid>",
                    "ed25519",
                    "sign",
                    "0",
                )
                listing = release.command(
                    "gpg", "--homedir", str(keys), "--with-colons", "--list-keys"
                )
                fingerprint = next(
                    line.split(":")[9]
                    for line in listing.splitlines()
                    if line.startswith("fpr:")
                )
                key = release.command(
                    "gpg",
                    "--homedir",
                    str(keys),
                    "--armor",
                    "--export-secret-keys",
                    fingerprint,
                )
                with (
                    patch.object(release, "ref_sha", return_value=None),
                    patch.dict(
                        os.environ,
                        {
                            "GPG_SECRET_KEY": key,
                            "SOURCE_SIGNING_FINGERPRINT": fingerprint,
                        },
                    ),
                ):
                    release.final_refs(release.Candidate("0.59.3-rc.2", sha))
                self.assertEqual(
                    release.command(
                        "git",
                        "--git-dir",
                        str(remote),
                        "rev-parse",
                        "refs/heads/releases/0.59.3",
                    ),
                    sha,
                )
                self.assertEqual(
                    release.command(
                        "git", "--git-dir", str(remote), "rev-parse", "v0.59.3^{commit}"
                    ),
                    sha,
                )
                with patch.dict(os.environ, {"GNUPGHOME": str(keys)}):
                    release.command("git", "verify-tag", "v0.59.3")
            finally:
                os.chdir(previous)
                subprocess.run(
                    ["gpgconf", "--homedir", str(keys), "--kill", "gpg-agent"],
                    check=False,
                )


if __name__ == "__main__":
    unittest.main()
