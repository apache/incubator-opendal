# Apache OpenDAL Website

This website is built using [Docusaurus 2](https://docusaurus.io/), a modern static website generator.

## Installation

```
$ pnpm install
```

## Local Development

```
$ pnpm start
```

This command starts a local development server and opens up a browser window. Most changes are reflected live without having to restart the server.

## Build

```
$ pnpm build
```

This command generates static content into the `build` directory and can be served using any static contents hosting service.

## LLM documentation

CI includes Rust documentation in `llms.txt` and `llms-full.txt`. Local builds include only website content by default.

To include Rust content locally, download the `rust-llms-documentation` artifact from a successful Docs workflow and set `OPENDAL_RUSTDOC_LLMS` to its `llms.json` file:

```bash
OPENDAL_RUSTDOC_LLMS=/tmp/opendal-rust-llms/llms.json pnpm build
```

## Images and badges

Website images are resolved locally by `plugins/local-images.js`. The resolver runs before Markdown compilation and on the generated API documentation, so client navigation and standalone API pages use the same resources. It does not download images during a build.

- Shared README and rustdoc sources use `https://opendal.apache.org/img/` URLs for project images. The website resolves them to files in `static/img/`, with the current website base URL.
- Badges are generated as self-contained SVG data URLs. Version badges show the version of the documented Rust, Node.js, or Ruby package, read from that package's manifest. CI, package-download, and Discord badges are fixed links rather than live metrics.
- Unknown external images fail the build with their page and URL. Add a local asset or an explicit badge presentation to the resolver instead of adding a network fallback.

Run `pnpm test` to check image paths in API HTML, independent package versions, and errors for unknown external images. To test a nested deployment path, run `OPENDAL_WEBSITE_BASE_URL=/opendal/opendal-docs-stable/ pnpm build`.

## Dependencies

Generate dependencies by `npx license-checker --production --excludePrivatePackages --csv > DEPENDENCIES.node.csv`


## For content search

Since search plugin can not work with `pnpm start`, for testing please run `pnpm build && pnpm serve`.
