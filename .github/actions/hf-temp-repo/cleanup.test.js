/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const test = require("node:test");
const vm = require("node:vm");

const source = readFileSync(require.resolve("./cleanup.js"), "utf8");

async function runCleanup(state) {
  const requests = [];
  await vm.runInNewContext(source, {
    require(name) {
      assert.equal(name, "./common");
      return {
        getState: (key) => state[key] || "",
        async hfRequest(method, path, token, body) {
          requests.push({ method, path, token, body: body && { ...body } });
        },
      };
    },
    console: {
      log() {},
      warn(message) {
        throw new Error(message);
      },
    },
  });
  return requests;
}

test("dataset cleanup sends the namespace and bare repository name", async () => {
  const requests = await runCleanup({
    repo_id: "opendal/test-dataset-123-abcd",
    repo_type: "dataset",
    token: "test-token",
  });
  assert.deepEqual(requests, [{
    method: "DELETE",
    path: "/api/repos/delete",
    token: "test-token",
    body: { type: "dataset", name: "test-dataset-123-abcd", organization: "opendal" },
  }]);
});

test("bucket cleanup uses the bucket endpoint", async () => {
  const requests = await runCleanup({
    repo_id: "opendal/test-bucket-123-abcd",
    repo_type: "bucket",
    token: "test-token",
  });
  assert.deepEqual(requests, [{
    method: "DELETE",
    path: "/api/buckets/opendal/test-bucket-123-abcd",
    token: "test-token",
    body: undefined,
  }]);
});

test("cleanup skips deletion when creation saved no repository", async () => {
  assert.deepEqual(await runCleanup({}), []);
});
