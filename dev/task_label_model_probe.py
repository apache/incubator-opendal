"""Exercise the candidate label-event path on public data without GitHub writes."""

import copy
import hashlib
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import label


class ReadOnlyGitHub(label.GitHub):
    def call(self, path, method="GET", body=None):
        assert method == "GET", "This probe must not mutate GitHub"
        return super().call(path, method, body)


original_request = label.request


def observed_request(url, token, method="GET", body=None):
    try:
        response = original_request(url, token, method, body)
    except RuntimeError as error:
        context = error.__context__
        if "/ai/v1/chat/completions" in url and hasattr(context, "read"):
            detail = context.read().decode("utf-8", errors="replace")[:4096]
            print(json.dumps({"provider_error": detail.replace(token, "[redacted]")}), flush=True)
        raise
    if "/ai/v1/chat/completions" in url:
        print(json.dumps({
            "response_keys": sorted(response),
            "finish_reasons": [choice.get("finish_reason") for choice in response.get("choices", [])],
            "usage": response.get("usage"),
        }), flush=True)
    return response


label.request = observed_request
github = ReadOnlyGitHub("apache/opendal")
assert "services/git" not in label.component_labels()
literal = label.model_request({"pull_request": {}, "title": "", "body": ""}, [])
literal["messages"] = [{"role": "user", "content": 'Return exactly {"category":"feat","components":[],"breaking_change":false}.'}]
for enum_present in (True, False):
    request_body = copy.deepcopy(literal)
    if enum_present:
        request_body["response_format"]["json_schema"]["schema"]["properties"]["components"]["items"]["enum"] = list(label.component_labels())
    try:
        diagnostic = label.classify(request_body)
    except ValueError as error:
        print(json.dumps({"literal_empty_array": True, "enum_present": enum_present, "error": str(error)}), flush=True)
        assert enum_present, "The candidate must handle an explicit empty-array request"
    else:
        print(json.dumps({"literal_empty_array": True, "enum_present": enum_present, "classification": diagnostic}), flush=True)
        if not enum_present:
            assert diagnostic == {"category": "feat", "components": [], "breaking_change": False}

cases = [
    (6836, "8641f2a9230b4806aa0706c9b3da744ac86bda65", {"category": "feat", "components": [], "breaking_change": False}),
    (8472, "0989d0adc1e4d9ce31759bd6a1838b27075aaa3b", {"category": "ci", "components": ["bindings/java"], "breaking_change": False}),
]
for number, expected_head, expected in cases:
    item = github.call(f"/issues/{number}")
    assert item["state"] == "open"
    assert github.call(f"/pulls/{number}")["head"]["sha"] == expected_head
    observed = []

    def infer(body):
        assert body["model"] == "@cf/meta/llama-3.3-70b-instruct-fp8-fast"
        assert body["max_tokens"] == 512 and body["temperature"] == 0
        data_sha = hashlib.sha256(body["messages"][1]["content"].encode()).hexdigest()
        if number == 6836:
            assert data_sha == "29184795dbcdb7a36746e7ae58d935cde7d3a4b154e4843de107acc22bdb22a7"
        print(json.dumps({"pr": number, "head": expected_head, "user_data_sha256": data_sha,
                          "request_sha256": hashlib.sha256(json.dumps(body, sort_keys=True).encode()).hexdigest()}), flush=True)
        result = label.classify(body)
        observed.append(result)
        print(json.dumps({"pr": number, "classification": result}), flush=True)
        return result

    label.label_event(github, {"pull_request": item}, infer, apply=False)
    assert observed == [expected], (number, observed)
    current = github.call(f"/issues/{number}")
    assert (current["title"], current.get("body")) == (item["title"], item.get("body"))
    assert github.call(f"/pulls/{number}")["head"]["sha"] == expected_head

historical = github.call("/issues/8010")
historical_head = github.call("/pulls/8010")["head"]["sha"]
historical_files = github.pages("/pulls/8010/files")
historical_result = label.classify(label.model_request(historical, historical_files))
label.selected_labels(historical_result, historical, historical_files)
assert historical_result["components"] == ["services/github"], historical_result
print(json.dumps({"pr": 8010, "historical_control": True, "head": historical_head,
                  "classification": historical_result}), flush=True)
current_historical = github.call("/issues/8010")
assert (current_historical["title"], current_historical.get("body")) == (historical["title"], historical.get("body"))
assert github.call("/pulls/8010")["head"]["sha"] == historical_head

print(json.dumps({"cases_passed": len(cases) + 1, "github_writes": 0, "model": label.MODEL}), flush=True)
print("STRUCTURED_LABEL_PROBE_PASSED", flush=True)
