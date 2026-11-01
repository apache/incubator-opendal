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

const path = require("node:path");
const fs = require("node:fs");
const { makeBadge } = require("badge-maker");
const cheerio = require("cheerio");
const { parse } = require("smol-toml");

module.exports = function localImages({ siteDir, baseUrl }) {
  const root = path.resolve(siteDir, "..");
  const manifests = [
    path.join(root, "core/Cargo.toml"),
    path.join(root, "bindings/nodejs/package.json"),
    path.join(root, "bindings/ruby/Cargo.toml"),
  ];
  const rust = parse(fs.readFileSync(manifests[0], "utf8")).workspace.package
    .version;
  const nodejs = JSON.parse(fs.readFileSync(manifests[1], "utf8")).version;
  const ruby = parse(fs.readFileSync(manifests[2], "utf8")).package.version;

  // These describe this documentation build, not the latest registry release
  // or a live metric. Keep the README's enclosing link as the live entry point.
  const badges = new Map([
    [
      "/badge/status-unreleased-red",
      { label: "status", message: "unreleased", color: "red" },
    ],
    [
      "/badge/status-released-blue",
      { label: "status", message: "released", color: "blue" },
    ],
    [
      "/badge/opendal-OpenDAL_Website-red",
      { label: "OpenDAL", message: "website", color: "blue" },
    ],
    [
      "/npm/v/opendal.svg",
      { label: "Node.js docs", message: `v${nodejs}`, color: "blue" },
    ],
    [
      "/gem/v/opendal",
      { label: "Ruby docs", message: `v${ruby}`, color: "blue" },
    ],
    [
      "/crates/v/opendal.svg",
      { label: "Rust docs", message: `v${rust}`, color: "blue" },
    ],
    [
      "/gem/dtv/opendal",
      { label: "RubyGems", message: "downloads", color: "blue" },
    ],
    [
      "/crates/d/opendal.svg",
      { label: "crates.io", message: "downloads", color: "blue" },
    ],
    [
      "/github/actions/workflow/status/apache/opendal/ci_core.yml",
      { label: "CI", message: "GitHub Actions", color: "blue" },
    ],
    [
      "/discord/1081052318650339399",
      { label: "chat", message: "Discord", color: "5865f2" },
    ],
  ]);
  const generated = new Map();

  function resolveImage(src) {
    if (!/^(https?:)?\/\//i.test(src)) return { src };
    const url = new URL(src, "https://opendal.apache.org");
    if (
      url.origin === "https://opendal.apache.org" &&
      url.pathname.startsWith("/img/")
    ) {
      const filename = path.join(siteDir, "static", url.pathname);
      if (!fs.statSync(filename).isFile()) {
        throw new Error(`Missing local image: ${filename}`);
      }
      return { src: `${baseUrl}${url.pathname.slice(1)}` };
    }

    let badge;
    if (url.origin === "https://img.shields.io") {
      badge = badges.get(url.pathname);
    } else if (url.href === "https://github.com/codespaces/badge.svg") {
      badge = { label: "GitHub", message: "Open in Codespaces", color: "blue" };
    }
    if (badge) {
      const key = JSON.stringify(badge);
      if (!generated.has(key)) {
        const svg = makeBadge(badge);
        generated.set(key, {
          src: `data:image/svg+xml;base64,${Buffer.from(svg).toString("base64")}`,
          alt: `${badge.label}: ${badge.message}`,
        });
      }
      return generated.get(key);
    }
    throw new Error(
      `Unsupported external image: ${src}. Add a local source in plugins/local-images.js.`,
    );
  }

  return {
    // Rewrite Markdown before compilation so hydration and client navigation
    // use the same local images as the server-rendered page.
    rehype() {
      return (tree, file) => {
        const nodes = [tree];
        while (nodes.length) {
          const node = nodes.pop();
          if (
            node.type === "element" &&
            node.tagName === "img" &&
            node.properties?.src
          ) {
            try {
              Object.assign(node.properties, resolveImage(node.properties.src));
            } catch (error) {
              file.fail(error.message, node);
            }
          }
          if (
            (node.type === "mdxJsxFlowElement" ||
              node.type === "mdxJsxTextElement") &&
            node.name === "img"
          ) {
            const src = node.attributes.find(
              (attribute) => attribute.name === "src",
            );
            if (typeof src?.value === "string") {
              try {
                const replacement = resolveImage(src.value);
                src.value = replacement.src;
                if (replacement.alt) {
                  const alt = node.attributes.find(
                    (attribute) => attribute.name === "alt",
                  );
                  if (alt) alt.value = replacement.alt;
                  else
                    node.attributes.push({
                      type: "mdxJsxAttribute",
                      name: "alt",
                      value: replacement.alt,
                    });
                }
              } catch (error) {
                file.fail(error.message, node);
              }
            }
          }
          if (node.children) nodes.push(...node.children);
        }
      };
    },

    plugin() {
      return {
        name: "opendal-local-images",
        async postBuild({ outDir }) {
          // API documentation is built by rustdoc, Doxygen, TypeDoc and YARD,
          // then copied into the website without going through MDX.
          const directories = [outDir];
          const failures = [];
          while (directories.length) {
            const directory = directories.pop();
            for (const entry of await fs.promises.readdir(directory, {
              withFileTypes: true,
            })) {
              const filename = path.join(directory, entry.name);
              if (entry.isDirectory()) {
                directories.push(filename);
                continue;
              }
              if (!entry.name.endsWith(".html")) continue;
              const html = await fs.promises.readFile(filename, "utf8");
              if (!/<img\b[^>]*\bsrc\s*=\s*["']?(?:https?:)?\/\//i.test(html))
                continue;
              const $ = cheerio.load(html);
              let modified = false;
              $("img[src]").each((_, img) => {
                const element = $(img);
                const src = element.attr("src");
                try {
                  const replacement = resolveImage(src);
                  if (replacement.src !== src) {
                    element.attr(replacement);
                    modified = true;
                  }
                } catch (error) {
                  failures.push(
                    `${path.relative(outDir, filename)}: ${error.message}`,
                  );
                }
              });
              if (modified) await fs.promises.writeFile(filename, $.html());
            }
          }
          if (failures.length) {
            throw new Error(
              `External images must have local sources:\n${failures.join("\n")}`,
            );
          }
        },
      };
    },
  };
};
