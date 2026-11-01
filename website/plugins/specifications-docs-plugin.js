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
const path = require("path");
const { execFileSync } = require("child_process");

const REPOSITORY_DIR = path.resolve(__dirname, "../..");
const SPECIFICATIONS_DIR = path.resolve(
  REPOSITORY_DIR,
  "core/core/src/docs/specs"
);

let markdownParser;
const lastUpdates = new Map();

async function getMarkdownParser() {
  if (!markdownParser) {
    markdownParser = Promise.all([
      import("remark-gfm"),
      import("remark-parse"),
      import("unified"),
    ]).then(([{ default: remarkGfm }, { default: remarkParse }, { unified }]) =>
      unified().use(remarkParse).use(remarkGfm)
    );
  }
  return markdownParser;
}

function resolveSpecification(relativePath) {
  const resolved = path.resolve(SPECIFICATIONS_DIR, relativePath);
  if (
    path.extname(resolved) !== ".md" ||
    !resolved.startsWith(`${SPECIFICATIONS_DIR}${path.sep}`)
  ) {
    throw new Error(
      `Specification include must reference a Markdown file under ${SPECIFICATIONS_DIR}: ${relativePath}`
    );
  }
  return resolved;
}

function isSpecificationInclude(node) {
  if (node.type !== "mdxJsxFlowElement" || node.name !== "Specification") {
    return false;
  }
  if (node.attributes.length !== 0) {
    throw new Error("Specification include does not accept attributes");
  }
  return true;
}

async function includeSpecifications(node, source) {
  if (!Array.isArray(node.children)) {
    return 0;
  }

  const parser = await getMarkdownParser();
  let includeCount = 0;
  for (let index = 0; index < node.children.length; index += 1) {
    const child = node.children[index];
    if (isSpecificationInclude(child)) {
      const included = parser.parse(fs.readFileSync(source, "utf8"));
      node.children.splice(index, 1, ...included.children);
      index += included.children.length - 1;
      includeCount += 1;
    } else {
      includeCount += await includeSpecifications(child, source);
    }
  }
  return includeCount;
}

function remarkIncludeSpecification() {
  return async (tree, file) => {
    const relativePath = file.data.frontMatter.specification_source;
    if (relativePath === undefined) {
      return;
    }
    if (typeof relativePath !== "string") {
      throw new Error(`Specification source must be a string: ${file.path}`);
    }
    const source = resolveSpecification(relativePath);
    const includeCount = await includeSpecifications(tree, source);
    if (includeCount !== 1) {
      throw new Error(
        `Specification page must contain exactly one include: ${file.path}`
      );
    }
  };
}

function readLastUpdate(source) {
  if (!lastUpdates.has(source)) {
    let lastUpdate;
    try {
      const relativePath = path.relative(REPOSITORY_DIR, source);
      const output = execFileSync(
        "git",
        ["log", "-1", "--format=%aN%x00%aI", "--", relativePath],
        { cwd: REPOSITORY_DIR, encoding: "utf8" }
      ).trimEnd();
      if (output) {
        const [author, date] = output.split("\0");
        lastUpdate = { author, date };
      }
    } catch {
      lastUpdate = undefined;
    }
    lastUpdates.set(source, lastUpdate);
  }
  return lastUpdates.get(source);
}

async function parseSpecificationFrontMatter({
  filePath,
  fileContent,
  defaultParseFrontMatter,
}) {
  const parsed = await defaultParseFrontMatter({ filePath, fileContent });
  const relativePath = parsed.frontMatter.specification_source;
  if (relativePath === undefined) {
    return parsed;
  }
  if (typeof relativePath !== "string") {
    throw new Error(`Specification source must be a string: ${filePath}`);
  }
  // The wrapper owns website metadata, but the canonical file owns its history.
  const lastUpdate = readLastUpdate(resolveSpecification(relativePath));
  if (lastUpdate) {
    parsed.frontMatter.last_update = lastUpdate;
  }
  return parsed;
}

module.exports = function specificationsDocsPlugin() {
  return {
    name: "opendal-specifications-docs",
    getPathsToWatch() {
      return [path.join(SPECIFICATIONS_DIR, "**/*.md")];
    },
  };
};

module.exports.remarkIncludeSpecification = remarkIncludeSpecification;
module.exports.parseSpecificationFrontMatter = parseSpecificationFrontMatter;
