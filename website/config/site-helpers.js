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

const exec = require("child_process").execSync;
const fs = require("node:fs");
const semver = require("semver");

function envValue(name, fallback) {
  return process.env[name] ? process.env[name] : fallback;
}

function getWebsiteVersion(websiteStaging) {
  if (websiteStaging && process.env.GITHUB_REF_TYPE === "tag") {
    const refName = process.env.GITHUB_REF_NAME;
    if (refName?.startsWith("v")) {
      const version = semver.parse(refName, {}, true);
      return `${version.major}.${version.minor}.${version.patch}`;
    }
  }

  try {
    const refName = exec(
      "git describe --tags --abbrev=0 --match 'v*' --exclude '*rc*'"
    ).toString();
    const version = semver.parse(refName, {}, true);
    return `${version.major}.${version.minor}.${version.patch}`;
  } catch {
    console.warn("Failed to get version from Git, using default '0.0.0'");
    return "0.0.0";
  }
}

function getWebsiteSettings() {
  const baseUrl = envValue("OPENDAL_WEBSITE_BASE_URL", "/");
  const websiteNotLatest = envValue("OPENDAL_WEBSITE_NOT_LATEST", false);
  const websiteStaging = envValue("OPENDAL_WEBSITE_STAGING", false);

  return {
    baseUrl,
    websiteNotLatest,
    websiteStaging,
    websiteVersion: getWebsiteVersion(websiteStaging),
  };
}

function createLegacyDocsRedirect(existingPath) {
  if (existingPath.startsWith("/docs/") && existingPath !== "/docs/") {
    const legacy = existingPath.replace(/^\/docs\//, "/");
    if (legacy !== "/download/") {
      return [legacy];
    }
  }
  return undefined;
}

function addRustdocLlmSessions(ctx, baseUrl = "/") {
  const filename = process.env.OPENDAL_RUSTDOC_LLMS;
  if (!filename) return;

  const config = JSON.parse(fs.readFileSync(filename, "utf8"));
  if (!config?.sessions?.length || !config?.fullSessions?.length) {
    throw new Error(`Rust LLM documentation is empty: ${filename}`);
  }

  const rustdocUrl = `https://opendal.apache.org${baseUrl}docs/rust/`;
  ctx.llmConfig.llmStdConfig.sessions.unshift({
    sessionName: config.libName,
    source: "normal",
    items: config.sessions.map((item) => ({
      title: item.title,
      description: item.description,
      link: `${rustdocUrl}${item.link}`,
    })),
  });
  ctx.llmConfig.llmFullStdConfig.sessions = config.fullSessions
    .map((item) => ({
      link: `${rustdocUrl}${item.link}`,
      content: item.content,
    }))
    .concat(ctx.llmConfig.llmFullStdConfig.sessions);
  console.log(
    `Loaded Rust LLM documentation: ${config.sessions.length} index entries, ${config.fullSessions.length} full-text entries`,
  );
}

function orderByPathDepth(a, b) {
  const aSegments = new URL(a).pathname.split("/").filter(Boolean);
  const bSegments = new URL(b).pathname.split("/").filter(Boolean);

  if (aSegments.length !== bSegments.length) {
    return aSegments.length - bSegments.length;
  }

  return a.localeCompare(b);
}

function applyGeneratedConfig(config, { baseUrl, websiteNotLatest }) {
  config.baseUrl = baseUrl;

  if (websiteNotLatest && config.themeConfig) {
    config.themeConfig.announcementBar = {
      id: "announcementBar-0",
      content:
        'You are viewing the documentation of a <strong>historical release</strong>. <a href="https://nightlies.apache.org/opendal/opendal-docs-stable/">View the latest stable release</a>.',
    };
  }

  return config;
}

module.exports = {
  addRustdocLlmSessions,
  applyGeneratedConfig,
  createLegacyDocsRedirect,
  getWebsiteSettings,
  orderByPathDepth,
};
