#!/usr/bin/env python3
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

import json
import os
import urllib.error
import urllib.parse
import urllib.request
from contextlib import contextmanager
from collections.abc import Iterator


DEFAULT_REGISTRY_URL = "https://crates.io"
USER_AGENT = "apache-opendal-release/1.0 (https://github.com/apache/opendal)"


def _response_error(operation: str, error: urllib.error.HTTPError) -> RuntimeError:
    try:
        response = error.read().decode("utf-8", errors="replace")
    finally:
        error.close()
    try:
        details = json.loads(response)
        errors = details.get("errors", [])
        response = "; ".join(error["detail"] for error in errors)
    except (json.JSONDecodeError, AttributeError, KeyError, TypeError):
        pass
    return RuntimeError(f"{operation} failed with HTTP {error.code}: {response}")


def _request_json(request: urllib.request.Request, operation: str) -> dict[str, object]:
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        raise _response_error(operation, error) from None
    except urllib.error.URLError as error:
        raise RuntimeError(f"{operation} failed: {error.reason}") from None


def _oidc_request_url(request_url: str, audience: str) -> str:
    parsed = urllib.parse.urlsplit(request_url)
    query = urllib.parse.parse_qsl(parsed.query, keep_blank_values=True)
    query.append(("audience", audience))
    return urllib.parse.urlunsplit(
        (
            parsed.scheme,
            parsed.netloc,
            parsed.path,
            urllib.parse.urlencode(query),
            parsed.fragment,
        )
    )


def _mask_secret(value: str) -> None:
    if os.environ.get("GITHUB_ACTIONS") == "true":
        print(f"::add-mask::{value}", flush=True)


# The GitHub Action runs once per step. This helper repeats the same exchange
# and revoke protocol around every cargo publish attempt.
def request_trusted_publishing_token(
    registry_url: str = DEFAULT_REGISTRY_URL,
) -> str:
    request_url = os.environ.get("ACTIONS_ID_TOKEN_REQUEST_URL")
    request_token = os.environ.get("ACTIONS_ID_TOKEN_REQUEST_TOKEN")
    if not request_url or not request_token:
        raise RuntimeError(
            "GitHub OIDC is unavailable; grant this job `id-token: write`"
        )

    audience = registry_url.rstrip("/")
    for prefix in ("https://", "http://"):
        if audience.startswith(prefix):
            audience = audience.removeprefix(prefix)
            break
    oidc_request = urllib.request.Request(
        _oidc_request_url(request_url, audience),
        headers={
            "Authorization": f"Bearer {request_token}",
            "User-Agent": USER_AGENT,
        },
    )
    oidc_response = _request_json(oidc_request, "requesting a GitHub OIDC token")
    jwt = oidc_response.get("value")
    if not isinstance(jwt, str) or not jwt:
        raise RuntimeError("GitHub OIDC response did not contain a token")
    _mask_secret(jwt)

    token_request = urllib.request.Request(
        f"{registry_url.rstrip('/')}/api/v1/trusted_publishing/tokens",
        data=json.dumps({"jwt": jwt}).encode(),
        headers={
            "Content-Type": "application/json",
            "User-Agent": USER_AGENT,
        },
        method="POST",
    )
    token_response = _request_json(
        token_request, "exchanging a crates.io Trusted Publishing token"
    )
    token = token_response.get("token")
    if not isinstance(token, str) or not token:
        raise RuntimeError(
            "crates.io Trusted Publishing response did not contain a token"
        )
    _mask_secret(token)
    return token


def revoke_trusted_publishing_token(
    token: str, registry_url: str = DEFAULT_REGISTRY_URL
) -> None:
    request = urllib.request.Request(
        f"{registry_url.rstrip('/')}/api/v1/trusted_publishing/tokens",
        headers={
            "Authorization": f"Bearer {token}",
            "User-Agent": USER_AGENT,
        },
        method="DELETE",
    )
    try:
        with urllib.request.urlopen(request, timeout=30):
            return
    except urllib.error.HTTPError as error:
        raise _response_error(
            "revoking a crates.io Trusted Publishing token", error
        ) from None
    except urllib.error.URLError as error:
        raise RuntimeError(
            f"revoking a crates.io Trusted Publishing token failed: {error.reason}"
        ) from None


@contextmanager
def temporary_trusted_publishing_token(
    registry_url: str = DEFAULT_REGISTRY_URL,
) -> Iterator[str]:
    token = request_trusted_publishing_token(registry_url)
    try:
        yield token
    finally:
        revoke_trusted_publishing_token(token, registry_url)
