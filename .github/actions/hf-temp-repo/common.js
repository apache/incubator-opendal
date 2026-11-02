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

const fs = require("fs");
const https = require("https");

function hfRequest(method, path, token, body) {
  return new Promise((resolve, reject) => {
    const data = body ? JSON.stringify(body) : null;
    const req = https.request(
      {
        hostname: "huggingface.co",
        path,
        method,
        headers: {
          Authorization: `Bearer ${token}`,
          ...(data && {
            "Content-Type": "application/json",
            "Content-Length": Buffer.byteLength(data),
          }),
        },
      },
      (res) => {
        let result = "";
        res.on("data", (chunk) => (result += chunk));
        res.on("end", () => {
          if (res.statusCode >= 200 && res.statusCode < 300) {
            resolve(result);
          } else {
            // Prefer the structured error header to avoid logging large HTML pages.
            const msg =
              res.headers["x-error-message"] ||
              result.slice(0, 200) ||
              "unknown error";
            reject(new Error(`HTTP ${res.statusCode}: ${msg}`));
          }
        });
      }
    );
    req.on("error", reject);
    if (data) req.write(data);
    req.end();
  });
}

function getInput(name) {
  return process.env[`INPUT_${name.toUpperCase()}`] || "";
}

function getState(name) {
  return process.env[`STATE_${name}`] || "";
}

function saveState(name, value) {
  fs.appendFileSync(process.env.GITHUB_STATE, `${name}=${value}\n`);
}

function exportVariable(name, value) {
  fs.appendFileSync(process.env.GITHUB_ENV, `${name}=${value}\n`);
}

module.exports = { hfRequest, getInput, getState, saveState, exportVariable };
