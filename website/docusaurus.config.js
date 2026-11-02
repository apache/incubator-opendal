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
// Note: type annotations allow type checking and IDEs autocompletion

const { themes } = require("prism-react-renderer");
const {
  addRustdocLlmSessions,
  applyGeneratedConfig,
  createLegacyDocsRedirect,
  getWebsiteSettings,
  orderByPathDepth,
} = require("./config/site-helpers");
const {
  parseSpecificationFrontMatter,
  remarkIncludeSpecification,
} = require("./plugins/specifications-docs-plugin");

const repoAddress = "https://github.com/apache/opendal";

const { baseUrl, websiteNotLatest, websiteStaging, websiteVersion } =
  getWebsiteSettings();
const localImages = require("./plugins/local-images")({
  siteDir: __dirname,
  baseUrl,
});

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: "Apache OpenDAL™",
  tagline:
    "Open Data Access Layer: Access data freely, painlessly, and efficiently",
  favicon: "img/favicon.ico",

  customFields: {
    isStaging: websiteStaging,
    version: websiteVersion,
  },

  url: "https://opendal.apache.org/",
  baseUrl: "/",

  // Always set trailingSlash to true to avoid redirecting to a URL with a trailing slash
  trailingSlash: true,

  onBrokenLinks: "throw",
  markdown: {
    format: "detect",
    parseFrontMatter: parseSpecificationFrontMatter,
    hooks: {
      onBrokenMarkdownLinks: "throw",
    }
  },

  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  future: {
    faster: {
      rspackBundler: true,
      rspackPersistentCache: true,
    },
  },


  presets: [
    [
      "@docusaurus/preset-classic",
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          routeBasePath: "docs",
          sidebarPath: require.resolve("./docs/sidebars.js"),
          editUrl: "https://github.com/apache/opendal/tree/main/website/",
          showLastUpdateAuthor: true,
          showLastUpdateTime: true,
          beforeDefaultRemarkPlugins: [remarkIncludeSpecification],
          remarkPlugins: [require("./plugins/remark-include-code")],
          rehypePlugins: [localImages.rehype],
        },
        blog: {
          showReadingTime: true,
          editUrl: "https://github.com/apache/opendal/tree/main/website/",
          onUntruncatedBlogPosts: "warn",
          rehypePlugins: [localImages.rehype],
        },
        pages: {
          rehypePlugins: [localImages.rehype],
        },
        theme: {
          customCss: [
            require.resolve("./src/css/variables.css"),
            require.resolve("./src/css/global.css"),
            require.resolve("./src/css/navbar.css"),
            require.resolve("./src/css/search.css"),
            require.resolve("./src/css/sidebar.css"),
            require.resolve("./src/css/footer.css"),
          ],
        },
        sitemap: {
          changefreq: "daily",
          priority: 0.5,
          ignorePatterns: ["/tags/**"],
          filename: "sitemap.xml",
        },
      }),
    ],
  ],

  plugins: [
    // Keep a single @docusaurus/theme-common instance so navbar hooks share
    // the same React context as theme-classic providers under pnpm.
    require.resolve("./plugins/theme-common-singleton-plugin"),
    require.resolve("./plugins/specifications-docs-plugin"),
    [
      "@docusaurus/plugin-content-docs",
      {
        id: "community",
        path: "community",
        routeBasePath: "community",
        sidebarPath: require.resolve("./community/sidebars.js"),
        editUrl: "https://github.com/apache/opendal/tree/main/website/",
        rehypePlugins: [localImages.rehype],
      },
    ],
    [require.resolve("docusaurus-plugin-image-zoom"), {}],
    [
      "@docusaurus/plugin-client-redirects",
      {
        redirects: [
          {
            from: "/discord",
            to: "https://discord.com/invite/XQy8yGR2dg",
          },
          {
            from: "/maillist",
            to: "https://lists.apache.org/list.html?dev@opendal.apache.org",
          },
        ],
        createRedirects: createLegacyDocsRedirect,
      },
    ],
    require.resolve("docusaurus-lunr-search"),
    // Generates the /services section from data/services.json.
    require.resolve("./plugins/services-docs-plugin"),
    localImages.plugin,
    [
      "docusaurus-plugin-llms-builder",
      /** @type {import("docusaurus-plugin-llms-builder").PluginOptions} */
      ({
        version: websiteVersion,
        llmConfigs: [
          {
            title: "Apache OpenDAL™: One Layer, All Storage.",
            description:
              'OpenDAL (/ˈoʊ.pən.dæl/, pronounced "OH-puhn-dal") is an Open Data Access Layer that enables seamless interaction with diverse storage services.',
            summary:
              "OpenDAL's development is guided by its vision of One Layer, All Storage and its core principles: Open Community, Solid Foundation, Fast Access, Object Storage First, and Extensible Architecture. Read the explained vision at OpenDAL Vision.",
            generateLLMsTxt: true,
            generateLLMsFullTxt: true,
            hooks: {
              "generate:prepare": (ctx) => {
                addRustdocLlmSessions(ctx, baseUrl);
              },
            },
            sessions: [
              {
                type: "docs",
                docsDir: "docs",
                sessionName: "Docs",
                sitemap: "sitemap.xml",
                patterns: {
                  ignorePatterns: ["**/blog/**", "**/blog", "**/community/**"],
                  orderPatterns: orderByPathDepth,
                },
              },
              {
                type: "blog",
                docsDir: "blog",
                sessionName: "Blog",
                rss: "rss.xml",
              },
              {
                type: "docs",
                docsDir: "community",
                sessionName: "Community",
              },
            ],
          },
        ],
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      // TODO social card image
      // image: 'img/opendal-social-card.jpg',
      colorMode: {
        defaultMode: "light",
        disableSwitch: false,
        respectPrefersColorScheme: true,
      },
      navbar: {
        logo: {
          alt: "Apache OpenDAL",
          src: "img/logo.svg",
          srcDark: "img/logo_dark.svg",
          href: "/",
          target: "_self",
          height: 32,
        },
        items: [
          {
            type: "doc",
            docId: "overview",
            position: "right",
            label: "Docs",
          },
          {
            to: "/services",
            label: "Services",
            position: "right",
          },
          {
            to: "/blog",
            label: "Blog",
            position: "right",
          },
          {
            type: "doc",
            docId: "community",
            position: "right",
            label: "Community",
            docsPluginId: "community",
          },
          {
            to: "/download",
            label: "Download",
            position: "right",
          },
          {
            type: "dropdown",
            label: "ASF",
            position: "right",
            items: [
              {
                label: "Foundation",
                to: "https://www.apache.org/",
              },
              {
                label: "License",
                to: "https://www.apache.org/licenses/",
              },
              {
                label: "Events",
                to: "https://www.apache.org/events/current-event.html",
              },
              {
                label: "Privacy",
                to: "https://privacy.apache.org/policies/privacy-policy-public.html",
              },
              {
                label: "Security",
                to: "https://www.apache.org/security/",
              },
              {
                label: "Sponsorship",
                to: "https://www.apache.org/foundation/sponsorship.html",
              },
              {
                label: "Thanks",
                to: "https://www.apache.org/foundation/thanks.html",
              },
              {
                label: "Code of Conduct",
                to: "https://www.apache.org/foundation/policies/conduct.html",
              },
            ],
          },
          {
            href: repoAddress,
            position: "right",
            className: "header-github-link",
            "aria-label": "GitHub repository",
          },
          {
            href: "https://discord.gg/XQy8yGR2dg",
            position: "right",
            className: "header-discord-link",
            "aria-label": "Discord",
          },
        ],
      },
      footer: {
        style: "light",
        logo: {
          alt: "Apache Software Foundation",
          src: "./img/asf_logo_wide.svg",
          href: "https://www.apache.org/",
          width: 300,
        },
        copyright: `Copyright © 2022-${new Date().getFullYear()}, The Apache Software Foundation<br/>Apache OpenDAL, OpenDAL, Apache, the Apache feather and the Apache OpenDAL project logo are either registered trademarks or trademarks of the Apache Software Foundation.`,
      },
      prism: {
        theme: themes.github,
        darkTheme: themes.dracula,
        additionalLanguages: ["rust", "java", "groovy", "python", "ruby", "cpp"],
      },
      zoom: {
        selector: "img:not(a img)",
        background: "rgba(255, 255, 255, 0.8)",
        config: {},
      },
    }),
};

module.exports = applyGeneratedConfig(config, { baseUrl, websiteNotLatest });
