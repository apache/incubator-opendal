"""Inspect one public PR's inference termination without changing labels."""

import copy
import hashlib
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import label


github = label.GitHub("apache/opendal")
item = github.call("/issues/6836")
head = github.call("/pulls/6836")["head"]["sha"]
assert head == "8641f2a9230b4806aa0706c9b3da744ac86bda65"
files = github.pages("/pulls/6836/files")
body = label.model_request(item, files)
assert "services/git" not in label.component_labels()
print(json.dumps({"head": head, "files": len(files), "request_sha256": hashlib.sha256(json.dumps(body, sort_keys=True).encode()).hexdigest()}), flush=True)


def infer(request_body):
    response = label.request(
        f"https://api.cloudflare.com/client/v4/accounts/{os.environ['CLOUDFLARE_ACCOUNT_ID']}/ai/run/{label.MODEL}",
        os.environ["CLOUDFLARE_API_TOKEN"],
        "POST",
        request_body,
    )
    assert response["success"]
    result = response["result"]
    choice = result["choices"][0]
    message = choice.get("message") or {}
    content = message.get("content") or ""
    summary = {
        "max_tokens": request_body["max_tokens"],
        "finish_reason": choice["finish_reason"],
        "usage": result.get("usage"),
        "message_keys": sorted(message),
        "content_chars": len(content),
        "reasoning_chars": len(message.get("reasoning_content") or message.get("reasoning") or ""),
    }
    try:
        parsed = json.loads(content)
    except json.JSONDecodeError:
        summary["valid_json"] = False
    else:
        summary["valid_json"] = True
        if choice["finish_reason"] == "stop":
            summary["selected_labels"] = sorted(label.selected_labels(parsed, item, files))
    print(json.dumps(summary), flush=True)
    return summary


baseline = infer(body)
if baseline["finish_reason"] != "length":
    raise RuntimeError("Token exhaustion was not reproduced; inspect before changing the budget")
candidate_body = copy.deepcopy(body)
candidate_body["max_tokens"] = 2048
candidate = infer(candidate_body)
assert candidate["finish_reason"] == "stop" and candidate["valid_json"]
current = github.call("/issues/6836")
assert (current["title"], current.get("body")) == (item["title"], item.get("body"))
assert github.call("/pulls/6836")["head"]["sha"] == head
print("BUDGET_PROBE_PASSED; no GitHub labels or other resources changed", flush=True)
