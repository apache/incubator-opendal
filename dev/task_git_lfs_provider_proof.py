#!/usr/bin/env python3
"""Bounded anonymous Git LFS provider proof for #6836.

This task-only script proves that the fixed public fixture is currently
available and that its LFS batch action can be streamed and verified. It does
not exercise OpenDAL.

The Git fetch is a single shallow fetch into an owned bare repository with
credential helpers, prompts, system config, and global config disabled. The
Git fetch is intentionally unauthenticated. The LFS batch POST is also
anonymous; it never reads or sends user credentials. Only the download action's
own header map is applied to the payload request. Cross-origin or
non-HTTPS redirects have Authorization, Proxy-Authorization, Cookie, Cookie2,
and Host headers removed before the redirected request is made.

The payload is read in fixed-size chunks and hashed incrementally. The batch
response is capped and is never logged. Signed action URLs, action tokens,
request bodies, response bodies, and raw transport errors are never printed or
persisted. The local nonce only isolates the temporary directory; the provider
is not asked to echo a nonce and no nonce is used for authentication.
"""

import hashlib
import json
import os
import re
import secrets
import shutil
import socket
import subprocess
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request


REPO_URL = "https://huggingface.co/hf-internal-testing/tiny-random-bert"
REPO_REF = "main"
POINTER_PATH = "pytorch_model.bin"

# Pinned from the read-only preflight. If upstream moves, fail instead of
# silently treating a different repository state as the same proof.
EXPECTED_COMMIT = "f171d7baecaf37b5da5a3616d8833b9969753535"
EXPECTED_POINTER_OID = (
    "9922e8996d0c7e24c7f4e7a5d9c5b7303549f4ee94de0f1138b103014b51be13"
)
EXPECTED_POINTER_SIZE = 540217

LFS_CONTENT_TYPE = "application/vnd.git-lfs+json"
PROOF_USER_AGENT = "opendal-git-lfs-provider-proof/1"

CHUNK_SIZE = 64 * 1024
BATCH_BODY_LIMIT = 1024 * 1024
GIT_INIT_TIMEOUT_SECONDS = 30
GIT_FETCH_TIMEOUT_SECONDS = 120
BATCH_TIMEOUT_SECONDS = 30
DOWNLOAD_TIMEOUT_SECONDS = 60

UNSAFE_REDIRECT_HEADERS = {
    "authorization",
    "cookie",
    "cookie2",
    "host",
    "proxy-authorization",
}


class ProofError(Exception):
    """A sanitized failure that is safe to report."""

    def __init__(self, reason, **fields):
        super().__init__(reason)
        self.reason = reason
        self.fields = fields


class SafeRedirectHandler(urllib.request.HTTPRedirectHandler):
    """Follow redirects, but do not forward authority across origins."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        newurl = newurl.replace(" ", "%20")
        old = urllib.parse.urlsplit(req.full_url)
        new = urllib.parse.urlsplit(newurl)

        if new.username is not None or new.password is not None:
            return None

        old_origin = (old.scheme, old.hostname, old.port)
        new_origin = (new.scheme, new.hostname, new.port)
        new_headers = dict(req.headers)
        if old_origin != new_origin or new.scheme != "https":
            for name in list(new_headers):
                if name.lower() in UNSAFE_REDIRECT_HEADERS:
                    del new_headers[name]

        method = req.get_method()
        data = req.data
        if code in (301, 302, 303) and method not in ("GET", "HEAD"):
            method = "GET"
            data = None

        return urllib.request.Request(
            newurl,
            data=data,
            headers=new_headers,
            origin_req_host=req.origin_req_host,
            unverifiable=True,
            method=method,
        )


def git_env():
    env = os.environ.copy()
    for name in list(env):
        if (
            name in {
                "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                "GIT_COMMON_DIR",
                "GIT_CONFIG_COUNT",
                "GIT_CONFIG_PARAMETERS",
                "GIT_CONFIG_SYSTEM",
                "GIT_DIR",
                "GIT_INDEX_FILE",
                "GIT_NAMESPACE",
                "GIT_OBJECT_DIRECTORY",
                "GIT_SSL_CAINFO",
                "GIT_SSL_CAPATH",
                "GIT_SSL_CERT",
                "GIT_SSL_KEY",
                "GIT_SSL_NO_VERIFY",
                "GIT_TRACE",
                "GIT_TRACE2",
                "GIT_TRACE2_EVENT",
                "GIT_TRACE2_PERF",
                "GIT_TRACE_CURL",
                "GIT_TRACE_CURL_NO_DATA",
                "GIT_TRACE_PACKET",
                "GIT_TRACE_PERFORMANCE",
                "GIT_TRACE_SETUP",
                "GIT_CURL_VERBOSE",
                "GIT_WORK_TREE",
            }
            or name.startswith("GIT_CONFIG_KEY_")
            or name.startswith("GIT_CONFIG_VALUE_")
        ):
            del env[name]
    env.update(
        {
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_ASKPASS": "/bin/false",
            "SSH_ASKPASS": "/bin/false",
            "GCM_INTERACTIVE": "never",
        }
    )
    return env


def run_git(args, step, timeout):
    try:
        completed = subprocess.run(
            ["git", *args],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=timeout,
            env=git_env(),
        )
    except FileNotFoundError:
        raise ProofError("git-unavailable")
    except subprocess.TimeoutExpired:
        raise ProofError("git-timeout", step=step)

    if completed.returncode != 0:
        raise ProofError("git-failed", step=step, exit_code=completed.returncode)
    return completed.stdout


def fetch_pointer(bare_dir):
    run_git(
        ["init", "--bare", "--quiet", bare_dir],
        step="init",
        timeout=GIT_INIT_TIMEOUT_SECONDS,
    )
    run_git(
        [
            "-c",
            "credential.helper=",
            "--git-dir",
            bare_dir,
            "fetch",
            "--depth=1",
            "--no-tags",
            "--quiet",
            REPO_URL,
            REPO_REF,
        ],
        step="fetch",
        timeout=GIT_FETCH_TIMEOUT_SECONDS,
    )

    commit_raw = run_git(
        ["--git-dir", bare_dir, "rev-parse", "FETCH_HEAD^{commit}"],
        step="rev-parse",
        timeout=GIT_INIT_TIMEOUT_SECONDS,
    )
    try:
        commit = commit_raw.decode("ascii").strip()
    except UnicodeDecodeError:
        raise ProofError("git-revision-invalid")

    if commit != EXPECTED_COMMIT:
        raise ProofError("fixture-commit-changed", commit=commit)

    pointer_raw = run_git(
        ["--git-dir", bare_dir, "cat-file", "-p", f"{commit}:{POINTER_PATH}"],
        step="cat-file",
        timeout=GIT_INIT_TIMEOUT_SECONDS,
    )
    return commit, pointer_raw


def parse_pointer(raw):
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError:
        raise ProofError("pointer-not-utf8")

    lines = text.splitlines()
    if len(lines) != 3:
        raise ProofError("pointer-invalid")
    if lines[0] != "version https://git-lfs.github.com/spec/v1":
        raise ProofError("pointer-invalid")

    oid_match = re.fullmatch(r"oid sha256:([0-9a-f]{64})", lines[1])
    size_match = re.fullmatch(r"size ([0-9]+)", lines[2])
    if oid_match is None or size_match is None:
        raise ProofError("pointer-invalid")

    oid = oid_match.group(1)
    size = int(size_match.group(1))
    if oid != EXPECTED_POINTER_OID or size != EXPECTED_POINTER_SIZE:
        raise ProofError(
            "fixture-pointer-changed",
            pointer_oid=f"sha256:{oid}",
            pointer_size=size,
        )
    return oid, size


def open_request(opener, request, timeout):
    try:
        return opener.open(request, timeout=timeout)
    except urllib.error.HTTPError as err:
        # The error body is intentionally not read or reported.
        raise ProofError("http-error", http_status=err.code)
    except urllib.error.URLError:
        raise ProofError("network-error")
    except (socket.timeout, TimeoutError):
        raise ProofError("network-timeout")


def batch_url():
    parsed = urllib.parse.urlsplit(REPO_URL)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
        or parsed.query
        or parsed.fragment
    ):
        raise ProofError("repository-url-invalid")

    base = REPO_URL.rstrip("/")
    if base.endswith(".git"):
        return f"{base}/info/lfs/objects/batch"
    return f"{base}.git/info/lfs/objects/batch"


def request_batch(opener, oid, size):
    body = json.dumps(
        {
            "operation": "download",
            "transfers": ["basic"],
            "objects": [{"oid": oid, "size": size}],
        },
        separators=(",", ":"),
    ).encode("utf-8")

    request = urllib.request.Request(
        batch_url(),
        data=body,
        method="POST",
        headers={
            "Accept": LFS_CONTENT_TYPE,
            "Content-Type": LFS_CONTENT_TYPE,
            "User-Agent": PROOF_USER_AGENT,
        },
    )

    with open_request(opener, request, BATCH_TIMEOUT_SECONDS) as response:
        status = response.getcode()
        if status is None or not 200 <= status < 300:
            raise ProofError("batch-http-status", http_status=status)
        raw = response.read(BATCH_BODY_LIMIT + 1)

    if len(raw) > BATCH_BODY_LIMIT:
        raise ProofError("batch-response-too-large")
    try:
        document = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        raise ProofError("batch-response-invalid")

    return extract_download_action(document, oid, size)


def extract_download_action(document, oid, size):
    try:
        objects = document["objects"]
        if not isinstance(objects, list) or len(objects) != 1:
            raise ProofError("download-action-missing")
        obj = objects[0]
        if not isinstance(obj, dict):
            raise ProofError("download-action-missing")
        if obj.get("oid", oid) != oid or obj.get("size", size) != size:
            raise ProofError("batch-object-mismatch")

        action = obj["actions"]["download"]
        href = action["href"]
        headers = action.get("header", {})
    except (KeyError, TypeError):
        raise ProofError("download-action-missing")

    if not isinstance(href, str) or not href:
        raise ProofError("download-action-invalid")
    parsed = urllib.parse.urlsplit(href)
    if (
        parsed.scheme != "https"
        or not parsed.hostname
        or parsed.username is not None
        or parsed.password is not None
    ):
        raise ProofError("download-action-invalid")

    if headers is None:
        headers = {}
    if not isinstance(headers, dict):
        raise ProofError("download-action-invalid")
    for name, value in headers.items():
        if not isinstance(name, str) or not isinstance(value, str):
            raise ProofError("download-action-invalid")

    return href, headers, parsed.hostname


def stream_payload(opener, href, headers, expected_size):
    request = urllib.request.Request(
        href,
        method="GET",
        headers=headers,
    )
    hasher = hashlib.sha256()
    total = 0
    chunks = 0

    with open_request(opener, request, DOWNLOAD_TIMEOUT_SECONDS) as response:
        status = response.getcode()
        if status is None or not 200 <= status < 300:
            raise ProofError("payload-http-status", http_status=status)

        while True:
            chunk = response.read(CHUNK_SIZE)
            if not chunk:
                break
            chunks += 1
            total += len(chunk)
            if total > expected_size:
                raise ProofError(
                    "payload-size-mismatch",
                    bytes=total,
                    pointer_size=expected_size,
                )
            hasher.update(chunk)

    return total, hasher.hexdigest(), chunks


def cleanup_temp(path):
    try:
        shutil.rmtree(path, ignore_errors=True)
        return "ok" if not os.path.exists(path) else "failed"
    except OSError:
        return "failed"


def run_proof(tmp_dir):
    bare_dir = os.path.join(tmp_dir, "repo.git")
    commit, pointer_raw = fetch_pointer(bare_dir)
    oid, size = parse_pointer(pointer_raw)

    opener = urllib.request.build_opener(SafeRedirectHandler())
    href, action_headers, action_host = request_batch(opener, oid, size)
    total, digest, chunks = stream_payload(opener, href, action_headers, size)

    if total != size or digest != oid:
        raise ProofError(
            "payload-mismatch",
            commit=commit,
            pointer_oid=f"sha256:{oid}",
            pointer_size=size,
            bytes=total,
            sha256=digest,
        )

    return {
        "commit": commit,
        "pointer_oid": f"sha256:{oid}",
        "pointer_size": size,
        "action_host": action_host,
        # Exactly two top-level LFS operations: one batch POST and one payload
        # GET. Redirects, if any, are part of the payload GET.
        "count": 2,
        "chunks": chunks,
        "bytes": total,
        "sha256": digest,
        "match": "true",
    }


def print_failure(reason, fields, cleanup):
    print("status=fail")
    print(f"reason={reason}")
    for name in sorted(fields):
        print(f"{name}={fields[name]}")
    print(f"cleanup={cleanup}")


def print_success(result, cleanup):
    print("status=pass")
    for name in (
        "commit",
        "pointer_oid",
        "pointer_size",
        "action_host",
        "count",
        "chunks",
        "bytes",
        "sha256",
        "match",
    ):
        print(f"{name}={result[name]}")
    print(f"cleanup={cleanup}")


def main():
    nonce = secrets.token_hex(8)
    try:
        tmp_dir = tempfile.mkdtemp(prefix=f"opendal-git-lfs-proof-{nonce}-")
    except OSError:
        print_failure("tempdir-unavailable", {}, "na")
        return 1

    result = None
    failure = None
    try:
        result = run_proof(tmp_dir)
    except ProofError as err:
        failure = err
    except Exception:
        # Do not print the exception: it may contain a signed URL or token.
        failure = ProofError("unexpected-error")
    finally:
        cleanup = cleanup_temp(tmp_dir)

    if cleanup != "ok":
        if failure is not None:
            print_failure(failure.reason, failure.fields, cleanup)
        else:
            print_failure("cleanup-failed", result or {}, cleanup)
        return 1

    if failure is not None:
        print_failure(failure.reason, failure.fields, cleanup)
        return 1

    print_success(result, cleanup)
    return 0


if __name__ == "__main__":
    sys.exit(main())
