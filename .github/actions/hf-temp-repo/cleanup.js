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

const { hfRequest, getState } = require("./common");

async function run() {
  const repoId = getState("repo_id");
  const repoType = getState("repo_type");
  const token = getState("token");

  if (!repoId) {
    console.log("No temp repo to clean up");
    return;
  }

  if (repoType === "bucket") {
    await hfRequest("DELETE", `/api/buckets/${repoId}`, token);
  } else {
    const [organization, name] = repoId.split("/");
    await hfRequest("DELETE", "/api/repos/delete", token, {
      type: repoType,
      name,
      organization,
    });
  }

  console.log(`Deleted temp ${repoType}: ${repoId}`);
}

run().catch((err) => console.warn(`Cleanup failed: ${err.message}`));
