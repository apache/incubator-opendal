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

// @ts-check

// A per-language guide: a collapsible category whose title links to the
// language's Overview, with the remaining guide pages as children. `dir` is the
// doc id prefix (e.g. 'core' or 'bindings/python'); `pages` lists the non-
// overview pages that language actually ships (omit those it doesn't have).
const langGuide = (label, dir, pages, icon) => ({
  type: 'category',
  label,
  // sidebar-lang-<key> drives the brand logo shown before the label (see
  // global.css). Defaults to the last path segment of the doc id prefix.
  className: `sidebar-lang sidebar-lang-${icon ?? dir.split('/').pop()}`,
  link: { type: 'doc', id: `${dir}/overview` },
  collapsed: true,
  items: pages.map((p) => `${dir}/${p}`),
});

// The full task-centric guide; thin/experimental bindings ship a subset.
const FULL = ['getting-started', 'connecting', 'tasks', 'production'];
const MIN = ['getting-started'];

/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docs: [
    // Concepts: read these first, regardless of language.
    'overview',
    'vision',
    'concepts',
    {
      type: 'category',
      label: 'Specifications',
      link: { type: 'doc', id: 'specifications/overview' },
      collapsed: true,
      items: [
        'specifications/conditional-operations',
        'specifications/metadata',
      ],
    },
    // Divider between the concept docs and the per-language guides.
    {
      type: 'html',
      value: '<div class="sidebar-section-label">Languages</div>',
      defaultStyle: false,
    },
    // Languages, flat. Rust (the core) leads; the rest are alphabetical.
    langGuide('Rust', 'core', FULL, 'rust'),
    langGuide('C', 'bindings/c', MIN),
    langGuide('C++', 'bindings/cpp', MIN),
    langGuide('D', 'bindings/d', MIN),
    langGuide('Dart', 'bindings/dart', MIN),
    langGuide('.NET', 'bindings/dotnet', FULL),
    langGuide('Go', 'bindings/go', FULL),
    langGuide('Haskell', 'bindings/haskell', MIN),
    langGuide('Java', 'bindings/java', FULL),
    langGuide('Lua', 'bindings/lua', MIN),
    langGuide('Node.js', 'bindings/nodejs', FULL),
    langGuide('OCaml', 'bindings/ocaml', MIN),
    langGuide('PHP', 'bindings/php', MIN),
    langGuide('Python', 'bindings/python', FULL),
    langGuide('Ruby', 'bindings/ruby', FULL),
    langGuide('Swift', 'bindings/swift', MIN),
    langGuide('Zig', 'bindings/zig', MIN),
  ],
};

module.exports = sidebars;
