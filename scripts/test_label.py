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

"""Label taxonomy, model boundary and event application contracts."""

import copy
import unittest
from unittest.mock import patch

import label


class LabelTests(unittest.TestCase):
    def item(self, pr=True):
        result = {
            "number": 1,
            "state": "open",
            "title": "fix: repair writes",
            "body": "A bug fix.",
            "labels": [],
        }
        if pr:
            result["pull_request"] = {}
        return result

    def test_catalog_covers_services_bindings_and_integrations(self):
        catalog = label.catalog()
        for name in (
            "services/goosefs",
            "services/huggingface",
            "services/memory",
            "services/cloudflare_kv",
            "bindings/zig",
            "bindings/moonbit",
            "integrations/parquet",
        ):
            self.assertIn(name, catalog)
        self.assertNotIn("services/hf", catalog)
        self.assertNotIn("services/azure_common", catalog)

    def test_schema_separates_issue_and_pr_categories(self):
        issue = label.model_request(self.item(False), [])
        pr = label.model_request(self.item(), [])
        schema = lambda body: body["response_format"]["json_schema"]["properties"]
        self.assertNotIn("fix", schema(issue)["category"]["enum"])
        self.assertNotIn("bug", schema(pr)["category"]["enum"])
        self.assertIn("services/goosefs", schema(pr)["components"]["items"]["enum"])
        self.assertNotIn("run-with-secrets", schema(pr)["components"]["items"]["enum"])

    def test_invalid_model_labels_never_reach_github(self):
        for result in (
            {
                "category": "fix",
                "components": ["run-with-secrets"],
                "breaking_change": False,
            },
            {
                "category": "fix",
                "components": ["services/invented"],
                "breaking_change": False,
            },
            {"category": "fix", "components": [], "breaking_change": "false"},
            {
                "category": "fix",
                "components": [],
                "breaking_change": False,
                "extra": "lgtm",
            },
        ):
            with self.subTest(result=result), self.assertRaises(ValueError):
                label.selected_labels(result, self.item(), [])

    def test_issue_cannot_receive_pr_category_or_breaking_flag(self):
        with self.assertRaises(ValueError):
            label.selected_labels(
                {"category": "fix", "components": []}, self.item(False), []
            )
        with self.assertRaises(ValueError):
            label.selected_labels(
                {"category": "bug", "components": [], "breaking_change": True},
                self.item(False),
                [],
            )

    def test_preserves_category_and_deduplicates_components(self):
        item = self.item()
        item["labels"] = [{"name": "releases-note/feat"}, {"name": "lgtm"}]
        labels = label.selected_labels(
            {
                "category": "fix",
                "components": ["services/goosefs"] * 2,
                "breaking_change": False,
            },
            item,
            [],
        )
        self.assertEqual(labels, {"services/goosefs", "size:XS"})

    def test_size_boundaries_exclude_lockfiles(self):
        for lines, size in [
            (9, "XS"),
            (10, "S"),
            (30, "M"),
            (100, "L"),
            (500, "XL"),
            (1000, "XXL"),
        ]:
            files = [
                {"filename": "core/src/lib.rs", "additions": lines, "deletions": 0},
                {"filename": "core/Cargo.lock", "additions": 10000, "deletions": 0},
            ]
            labels = label.selected_labels(
                {"category": "fix", "components": [], "breaking_change": False},
                self.item(),
                files,
            )
            self.assertIn("size:" + size, labels)

    def test_event_applies_only_current_item_and_replaces_size(self):
        item = self.item()
        item["labels"] = [
            {"name": "lgtm"},
            {"name": "releases-note/docs"},
            {"name": "size:XL"},
        ]
        github = FakeGitHub(item)
        with patch("builtins.print"):
            label.label_event(
                github,
                {"pull_request": item},
                lambda _: {
                    "category": "fix",
                    "components": ["services/goosefs"],
                    "breaking_change": False,
                },
                apply=True,
            )
        self.assertEqual(
            github.writes,
            [
                (
                    "/issues/1/labels",
                    "POST",
                    {"labels": ["services/goosefs", "size:XS"]},
                ),
                ("/issues/1/labels/size%3AXL", "DELETE", None),
            ],
        )

    def test_dry_run_makes_no_writes(self):
        item = self.item(False)
        github = FakeGitHub(item)
        with patch("builtins.print"):
            label.label_event(
                github,
                {"issue": item},
                lambda _: {"category": "bug", "components": ["core"]},
            )
        self.assertEqual(github.writes, [])

    def test_stale_head_or_body_cannot_apply_results(self):
        for changed in ("body", "head"):
            item = self.item()
            github = FakeGitHub(item)

            def infer(_, changed=changed, github=github):
                if changed == "body":
                    github.item["body"] = "Updated purpose"
                else:
                    github.head = "new"
                return {"category": "fix", "components": [], "breaking_change": False}

            with patch("builtins.print"):
                label.label_event(github, {"pull_request": item}, infer, apply=True)
            self.assertEqual(github.writes, [])

    def test_closed_item_is_skipped(self):
        item = self.item()
        item["state"] = "closed"
        github = FakeGitHub(item)
        label.label_event(
            github,
            {"pull_request": item},
            lambda _: self.fail("unexpected inference"),
            apply=True,
        )
        self.assertEqual(github.reads, [])

    def test_sync_only_creates_missing_labels(self):
        github = FakeGitHub(self.item())
        github.labels = [
            {"name": name} for name in label.catalog() if name != "services/goosefs"
        ]
        with patch("builtins.print"):
            label.sync_labels(github)
        self.assertEqual(len(github.writes), 1)
        self.assertEqual(github.writes[0][2]["name"], "services/goosefs")

    def test_cloudflare_response_is_parsed_and_truncation_rejected(self):
        response = {
            "success": True,
            "result": {
                "choices": [
                    {
                        "finish_reason": "stop",
                        "message": {"content": '{"category":"bug","components":[]}'},
                    }
                ]
            },
        }
        with (
            patch.dict(
                "os.environ",
                {"CLOUDFLARE_ACCOUNT_ID": "a" * 32, "CLOUDFLARE_API_TOKEN": "test"},
            ),
            patch.object(label, "request", return_value=response),
            patch("builtins.print"),
        ):
            self.assertEqual(label.classify({}), {"category": "bug", "components": []})
            response["result"]["choices"][0]["finish_reason"] = "length"
            with self.assertRaises(ValueError):
                label.classify({})


class FakeGitHub:
    def __init__(self, item):
        self.item = copy.deepcopy(item)
        self.head = "old"
        self.reads = []
        self.writes = []
        self.labels = []

    def call(self, path, method="GET", body=None):
        if method != "GET":
            self.writes.append((path, method, body))
            return None
        self.reads.append(path)
        if path.startswith("/pulls/"):
            return {"head": {"sha": self.head}}
        return copy.deepcopy(self.item)

    def pages(self, path):
        return self.labels if path == "/labels" else []


if __name__ == "__main__":
    unittest.main()
