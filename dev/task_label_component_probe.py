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
    (6836, "8641f2a9230b4806aa0706c9b3da744ac86bda65", "feat", []),
    (8472, "0989d0adc1e4d9ce31759bd6a1838b27075aaa3b", "fix", ["bindings/java"]),
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
        if number == 6836:
            assert hashlib.sha256(body["messages"][1]["content"].encode()).hexdigest() == "29184795dbcdb7a36746e7ae58d935cde7d3a4b154e4843de107acc22bdb22a7"
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

historical = github.call("/issues/8010")
historical_head = github.call("/pulls/8010")["head"]["sha"]
historical_files = github.pages("/pulls/8010/files")
result = label.classify(label.model_request(historical, historical_files))
selected = original_selected(result, historical, historical_files)
assert "services/github" in selected, result
print(json.dumps({"pr": 8010, "historical_control": True, "head": historical_head,
                  "proposals": result, "selected_components": sorted(selected.intersection(catalog))}), flush=True)
current = github.call("/issues/8010")
assert (current["title"], current.get("body")) == (historical["title"], historical.get("body"))
assert github.call("/pulls/8010")["head"]["sha"] == historical_head
print(json.dumps({"actual_cases_passed": 3, "github_writes": 0}), flush=True)
print("COMPONENT_PROPOSAL_PROBE_PASSED", flush=True)
