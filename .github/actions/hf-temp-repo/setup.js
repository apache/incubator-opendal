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

const { randomUUID } = require("crypto");

const { hfRequest, getInput, saveState, exportVariable } = require("./common");

async function run() {
  const repoPrefix = getInput("repo_id");
  const repoType = getInput("repo_type");
  const token = getInput("token");
  const repoId = `${repoPrefix}-${randomUUID().slice(0, 8)}`;
  const [organization, repoName] = repoId.split("/");

  if (repoType === "bucket") {
    await hfRequest("POST", `/api/buckets/${organization}/${repoName}`, token, {
      private: true,
    });
  } else {
    await hfRequest("POST", "/api/repos/create", token, {
      type: repoType,
      name: repoName,
      organization,
      private: true,
    });
  }

  console.log(`Created temp ${repoType}: ${repoId}`);
  saveState("repo_id", repoId);
  saveState("repo_type", repoType);
  saveState("token", token);
  exportVariable("OPENDAL_HF_REPO_ID", repoId);
}

run().catch((err) => {
  console.error(err.message);
  process.exit(1);
});
