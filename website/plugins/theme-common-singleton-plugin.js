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

const path = require("path");

/**
 * pnpm can install multiple physical copies of @docusaurus/theme-common when
 * peer dependency graphs diverge. Each copy creates its own React context, so
 * swizzled navbar code that imports the package from the site root can call
 * useNavbarMobileSidebar outside the NavbarMobileSidebarProvider that
 * theme-classic mounted. Force one resolve path for the whole bundle.
 */
module.exports = function themeCommonSingletonPlugin() {
  // package.json is not in "exports"; resolve the package entry then climb to root.
  const themeCommonRoot = path.resolve(
    path.dirname(require.resolve("@docusaurus/theme-common")),
    "..",
  );

  return {
    name: "theme-common-singleton-plugin",
    configureWebpack() {
      return {
        resolve: {
          alias: {
            "@docusaurus/theme-common/internal": path.join(
              themeCommonRoot,
              "lib/internal.js",
            ),
            "@docusaurus/theme-common/Details": path.join(
              themeCommonRoot,
              "lib/components/Details/index.js",
            ),
            "@docusaurus/theme-common": themeCommonRoot,
          },
        },
      };
    },
  };
};
