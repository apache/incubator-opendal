"""Validate component proposals against public cases without GitHub writes."""

import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import label


class ReadOnlyGitHub(label.GitHub):
    def call(self, path, method="GET", body=None):
        assert method == "GET", "This probe must not mutate GitHub"
        return super().call(path, method, body)


github = ReadOnlyGitHub("apache/opendal")
catalog = label.component_labels()
assert "services/git" not in catalog
original_selected = label.selected_labels
selections = []


def observed_selected(result, item, files):
    selected = original_selected(result, item, files)
    components = sorted(selected.intersection(catalog))
    selections.append(components)
    print(json.dumps({"pr": item["number"], "selected_components": components,
                      "selected_labels": sorted(selected)}), flush=True)
    return selected


label.selected_labels = observed_selected
cases = [
    (8473, "035a4855aa1d112b1ed6411de2fcb4a6b4c38e4a", "ci", []),
]
for number, expected_head, category, expected_components in cases:
    item = github.call(f"/issues/{number}")
    assert item["state"] == "open"
    assert github.call(f"/pulls/{number}")["head"]["sha"] == expected_head
    observed = []
    selections.clear()

    def infer(body):
        assert label.MODEL == "@cf/qwen/qwen3-30b-a3b-fp8"
        assert body["max_tokens"] == 512 and body["temperature"] == 0
        print(json.dumps({"pr": number, "head": expected_head,
                          "request_sha256": hashlib.sha256(json.dumps(body, sort_keys=True).encode()).hexdigest()}), flush=True)
        result = label.classify(body)
        observed.append(result)
        print(json.dumps({"pr": number, "proposals": result}), flush=True)
        return result

    label.label_event(github, {"pull_request": item}, infer, apply=False)
    assert len(observed) == 1 and observed[0]["category"] == category and observed[0]["breaking_change"] is False
    assert selections == [expected_components], selections
    current = github.call(f"/issues/{number}")
    assert (current["title"], current.get("body")) == (item["title"], item.get("body"))
    assert github.call(f"/pulls/{number}")["head"]["sha"] == expected_head

print(json.dumps({"actual_cases_passed": 1, "github_writes": 0}), flush=True)
print("COMPONENT_PROPOSAL_PROBE_PASSED", flush=True)
