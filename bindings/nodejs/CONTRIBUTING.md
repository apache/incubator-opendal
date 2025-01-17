# Contributing

- [Contributing](#contributing)
  - [Setup](#setup)
    - [Using a dev container environment](#using-a-dev-container-environment)
    - [Bring your own toolbox](#bring-your-own-toolbox)
  - [Build](#build)
  - [Test](#test)

## Setup

Building `nodejs` bindings requires some extra setup.

For small or first-time contributions, we recommend the dev container method. Prefer to do it yourself? That's fine too!

### Using a dev container environment

OpenDAL provides a pre-configured [dev container](https://containers.dev/) that could be used in [GitHub Codespaces](https://github.com/features/codespaces), [VSCode](https://code.visualstudio.com/), [JetBrains](https://www.jetbrains.com/remote-development/gateway/), [JupyterLab](https://jupyterlab.readthedocs.io/en/stable/). Please pick up your favourite runtime environment.

The fastest way is:

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/apache/opendal?quickstart=1&machine=standardLinux32gb)

### Bring your own toolbox

The `nodejs` binding requires `Node.js@16+` to be built. We recommend using the latest TLS version for development.

OpenDAL provides a `.node-version` file that specifies the recommended node versions. You can use any compatible tool to install the correct node version, such as [fnm](https://github.com/Schniz/fnm).

Alternatively, you can manually install the LTS node by following these steps:

For Ubuntu and Debian:

```shell
> curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash - && sudo apt-get install -y nodejs
```

For RHEL, CentOS, CloudLinux, Amazon Linux or Fedora:

```shell
> curl -fsSL https://rpm.nodesource.com/setup_lts.x | sudo bash -
```

Afterward, you will need to enable `corepack` to ensure that `pnpm` has been set up correctly:

```shell
> sudo corepack enable
```

To verify that everything is working properly, run `pnpm --version`:

```shell
> pnpm --version
8.11.0
```

### Using [devbox](https://www.jetify.com/devbox/docs/)

Launch Development Environment

```shell
devbox shell
```

Contains the following tools:

- `node`: `18`
- `pnpm`: `8.14.0`
- `libiconv`: fix nix `ld` missing issue

[Running Scripts](https://www.jetify.com/devbox/docs/guides/scripts/)

- `devbox run format`
- `devbox run test`
- `devbox run build`
- `devbox run dev`

## Build

```bash
# Install dependencies.
pnpm install
# Build from source.
pnpm build
# Build from source with debug info.
pnpm build:debug
```

## Test

We are using our own developed behavior testing framework.
Taking 'service-memory' as an example, you can use the following command to run it.

```bash
OPENDAL_TEST=memory pnpm test
```

which will output:

```
✓ |opendal| tests/service.test.mjs  (8 tests | 2 skipped) 40ms

 Test Files  1 passed (1)
      Tests  6 passed | 2 skipped (8)
   Start at  01:42:07
   Duration  233ms (transform 25ms, setup 0ms, collect 56ms, tests 40ms, environment 0ms, prepare 52ms)
```
