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

const fs = require("node:fs");
const path = require("node:path");

const [input, output] = process.argv.slice(2);
if (!input || !output) {
  throw new Error("Usage: generate-rustdoc-llms.cjs <rustdoc.json> <output.json>");
}

const docs = JSON.parse(fs.readFileSync(input, "utf8"));
const libName = docs.index[docs.root].name;
const sessions = [{ title: libName, description: "", link: `${libName}/` }];
const fullSessions = [];

// Read only documentation fields; unrelated Rustdoc schema changes (such as
// attributes changing from strings to objects) must not break the supplement.
for (const item of Object.values(docs.index)) {
  if (item.visibility !== "public" || !item.docs || !item.span) continue;

  const filename = item.span.filename.replace(/^src\//, "");
  const link = `src/${libName}/${filename}.html`;
  sessions.push({ title: item.name ?? filename, description: "", link });
  fullSessions.push({ content: item.docs, link });
}

if (!fullSessions.length) {
  throw new Error(`Rust LLM documentation is empty: ${input}`);
}
fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, JSON.stringify({ libName, sessions, fullSessions }));
console.log(
  `Generated ${sessions.length} index entries and ${fullSessions.length} full-text entries: ${output}`,
);
