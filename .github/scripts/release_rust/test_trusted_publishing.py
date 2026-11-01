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

import io
import json
import unittest
from unittest import mock
from urllib.parse import parse_qs
from urllib.parse import urlsplit

from trusted_publishing import _oidc_request_url
from trusted_publishing import request_trusted_publishing_token
from trusted_publishing import revoke_trusted_publishing_token
from trusted_publishing import temporary_trusted_publishing_token


class JsonResponse(io.BytesIO):
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.close()


class TrustedPublishingTest(unittest.TestCase):
    def test_oidc_request_url_preserves_existing_query(self):
        result = _oidc_request_url(
            "https://example.test/token?api-version=1", "crates.io"
        )

        query = parse_qs(urlsplit(result).query)
        self.assertEqual(query["api-version"], ["1"])
        self.assertEqual(query["audience"], ["crates.io"])

    def test_token_exchange_uses_github_oidc_and_crates_io(self):
        requests = []
        responses = iter(
            (
                JsonResponse(b'{"value":"github-jwt"}'),
                JsonResponse(b'{"token":"crates-token"}'),
            )
        )

        def urlopen(request, timeout):
            requests.append(request)
            self.assertEqual(timeout, 30)
            return next(responses)

        environment = {
            "ACTIONS_ID_TOKEN_REQUEST_URL": "https://github.test/oidc?api-version=1",
            "ACTIONS_ID_TOKEN_REQUEST_TOKEN": "request-token",
        }
        with (
            mock.patch.dict("trusted_publishing.os.environ", environment, clear=True),
            mock.patch("trusted_publishing.urllib.request.urlopen", urlopen),
        ):
            token = request_trusted_publishing_token()

        self.assertEqual(token, "crates-token")
        self.assertEqual(requests[0].get_method(), "GET")
        self.assertEqual(
            parse_qs(urlsplit(requests[0].full_url).query)["audience"],
            ["crates.io"],
        )
        self.assertEqual(
            requests[0].get_header("Authorization"), "Bearer request-token"
        )
        self.assertEqual(requests[1].get_method(), "POST")
        self.assertEqual(json.loads(requests[1].data), {"jwt": "github-jwt"})

    def test_revoke_uses_the_temporary_token(self):
        requests = []

        def urlopen(request, timeout):
            requests.append(request)
            self.assertEqual(timeout, 30)
            return JsonResponse(b"")

        with mock.patch("trusted_publishing.urllib.request.urlopen", urlopen):
            revoke_trusted_publishing_token("crates-token")

        self.assertEqual(requests[0].get_method(), "DELETE")
        self.assertEqual(requests[0].get_header("Authorization"), "Bearer crates-token")

    def test_temporary_token_is_always_revoked(self):
        with (
            mock.patch(
                "trusted_publishing.request_trusted_publishing_token",
                return_value="temporary-token",
            ),
            mock.patch("trusted_publishing.revoke_trusted_publishing_token") as revoke,
        ):
            with self.assertRaisesRegex(RuntimeError, "publish failed"):
                with temporary_trusted_publishing_token() as token:
                    self.assertEqual(token, "temporary-token")
                    raise RuntimeError("publish failed")

        revoke.assert_called_once_with("temporary-token", "https://crates.io")


if __name__ == "__main__":
    unittest.main()
