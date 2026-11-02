#!/usr/bin/env python3
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements. See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership. The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License. You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied. See the License for the
# specific language governing permissions and limitations
# under the License.

"""Verify one public Git/LFS fixture; this does not exercise OpenDAL."""

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request

REPOSITORY = "https://huggingface.co/hf-internal-testing/tiny-random-bert"
COMMIT = "f171d7baecaf37b5da5a3616d8833b9969753535"
OID = "9922e8996d0c7e24c7f4e7a5d9c5b7303549f4ee94de0f1138b103014b51be13"
SIZE = 540217
CHUNK = 65536


class DownloadRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, url):
        old, new = map(urllib.parse.urlsplit, (request.full_url, url))
        if new.scheme != "https" or new.username or new.password:
            raise ValueError("invalid redirect")
        redirected = super().redirect_request(request, fp, code, message, headers, url)
        if redirected and (old.hostname, old.port or 443) != (new.hostname, new.port or 443):
            for name in tuple(redirected.headers):
                if name.lower() in {"authorization", "proxy-authorization", "cookie", "host"}:
                    redirected.remove_header(name)
        return redirected


def main():
    report = {"case": "public_git_lfs_provider", "passed": False, "cases": 0}
    temporary = tempfile.TemporaryDirectory(prefix="opendal-pr6836-provider-")
    try:
        report["stage"] = "git"
        env = os.environ.copy()
        env.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
                   GIT_TERMINAL_PROMPT="0", GIT_ASKPASS="/bin/false")
        repository = os.path.join(temporary.name, "repo.git")

        def git(*args):
            return subprocess.check_output(
                ["git", "-c", "credential.helper=", "--git-dir", repository, *args],
                env=env, stderr=subprocess.DEVNULL, timeout=120,
            )

        git("init", "--bare", "--quiet")
        git("fetch", "--depth=1", "--no-tags", "--quiet", REPOSITORY, COMMIT)
        commit = git("rev-parse", "FETCH_HEAD^{commit}").decode().strip()
        pointer = git("cat-file", "-p", f"{commit}:pytorch_model.bin").decode()
        expected = f"version https://git-lfs.github.com/spec/v1\noid sha256:{OID}\nsize {SIZE}\n"
        if commit != COMMIT or pointer != expected:
            raise ValueError("immutable fixture mismatch")
        report.update(commit=commit, pointer_oid=OID, pointer_size=SIZE)

        report["stage"] = "batch"
        opener = urllib.request.build_opener(DownloadRedirects())
        request = urllib.request.Request(
            REPOSITORY + ".git/info/lfs/objects/batch",
            data=json.dumps({"operation": "download", "transfers": ["basic"],
                             "objects": [{"oid": OID, "size": SIZE}]}).encode(),
            headers={"Accept": "application/vnd.git-lfs+json",
                     "Content-Type": "application/vnd.git-lfs+json",
                     "User-Agent": "opendal-provider-validation/1"},
        )
        with opener.open(request, timeout=30) as response:
            body = response.read(CHUNK + 1)
        if len(body) > CHUNK:
            raise ValueError("oversized batch response")
        objects = json.loads(body)["objects"]
        if len(objects) != 1 or objects[0].get("oid") != OID or objects[0].get("size") != SIZE:
            raise ValueError("batch object mismatch")
        action = objects[0]["actions"]["download"]
        location = urllib.parse.urlsplit(action["href"])
        if location.scheme != "https" or not location.hostname or location.username or location.password:
            raise ValueError("invalid action URL")
        report.update(action_host=location.hostname,
                      action_expires_in_present="expires_in" in action,
                      action_expires_at_present="expires_at" in action)

        report["stage"] = "payload"
        request = urllib.request.Request(action["href"], headers=action.get("header", {}))
        digest, count, chunks = hashlib.sha256(), 0, 0
        with opener.open(request, timeout=60) as response:
            while data := response.read(CHUNK):
                count += len(data)
                chunks += 1
                if count > SIZE:
                    raise ValueError("oversized payload")
                digest.update(data)
        report.update(bytes=count, chunks=chunks, sha256=digest.hexdigest())
        if count != SIZE or digest.hexdigest() != OID:
            raise ValueError("payload mismatch")
        report.update(passed=True, cases=1, stage="complete")
    except Exception as error:
        # Error messages and bodies can contain signed URLs or action tokens.
        report["error_type"] = type(error).__name__
        if isinstance(error, urllib.error.HTTPError):
            report["http_status"] = error.code
        if isinstance(error, subprocess.CalledProcessError):
            report["git_exit"] = error.returncode
    finally:
        try:
            temporary.cleanup()
            report["cleanup"] = "complete"
        except OSError:
            report.update(passed=False, cleanup="failed")
    print(json.dumps(report, sort_keys=True))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
