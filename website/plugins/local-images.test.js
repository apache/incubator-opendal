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
const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const { test } = require("node:test");
const cheerio = require("cheerio");
const localImages = require("./local-images");

async function fixture(t, baseUrl = "/") {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "opendal-images-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const files = {
    "core/Cargo.toml": '[workspace.package]\nversion = "9.1.0"\n',
    "bindings/nodejs/package.json": '{"version":"2.3.4"}',
    "bindings/ruby/Cargo.toml": '[package]\nversion = "4.5.6"\n',
    "website/static/img/logo.svg": '<svg xmlns="http://www.w3.org/2000/svg"/>',
  };
  for (const [name, content] of Object.entries(files)) {
    const filename = path.join(root, name);
    await fs.mkdir(path.dirname(filename), { recursive: true });
    await fs.writeFile(filename, content);
  }
  const siteDir = path.join(root, "website");
  const outDir = path.join(siteDir, "build");
  await fs.mkdir(path.join(outDir, "docs/nodejs"), { recursive: true });
  return { ...localImages({ siteDir, baseUrl }), outDir };
}

for (const baseUrl of ["/", "/opendal/opendal-docs-stable/"]) {
  test(`API HTML localizes images and package versions under ${baseUrl}`, async (t) => {
    const { plugin, outDir } = await fixture(t, baseUrl);
    const filename = path.join(outDir, "docs/nodejs/index.html");
    await fs.writeFile(
      filename,
      `
      <a href="https://www.npmjs.com/package/opendal"><img id="nodejs"
        alt="Latest Version" src="https://img.shields.io/npm/v/opendal.svg?logo=npm"></a>
      <img id="rust" src="https://img.shields.io/crates/v/opendal.svg">
      <img id="ruby" src="https://img.shields.io/gem/v/opendal">
      <img id="logo" src="https://opendal.apache.org/img/logo.svg">
    `,
    );
    await plugin().postBuild({ outDir });
    const $ = cheerio.load(await fs.readFile(filename, "utf8"));
    assert.equal($("#logo").attr("src"), `${baseUrl}img/logo.svg`);
    assert.equal(
      $("#nodejs").parent().attr("href"),
      "https://www.npmjs.com/package/opendal",
    );
    for (const [id, expected] of [
      ["rust", "Rust docs: v9.1.0"],
      ["nodejs", "Node.js docs: v2.3.4"],
      ["ruby", "Ruby docs: v4.5.6"],
    ]) {
      assert.equal($(`#${id}`).attr("alt"), expected);
      assert.match($(`#${id}`).attr("src"), /^data:image\/svg\+xml;base64,/);
    }
  });
}

test("unknown external images fail with their page and URL", async (t) => {
  const { plugin, outDir } = await fixture(t);
  await fs.writeFile(
    path.join(outDir, "docs/nodejs/index.html"),
    '<img src="//example.com/unknown.png">',
  );
  await assert.rejects(
    plugin().postBuild({ outDir }),
    (error) =>
      error.message.includes("docs/nodejs/index.html") &&
      error.message.includes("//example.com/unknown.png"),
  );
});
