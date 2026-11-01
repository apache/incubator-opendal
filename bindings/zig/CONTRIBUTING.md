# Contributing

- [Contributing](#contributing)
  - [Setup](#setup)
    - [Using a dev container environment](#using-a-dev-container-environment)
    - [Bring your own toolbox](#bring-your-own-toolbox)
  - [Build](#build)
  - [Test](#test)

## Setup

### Using a dev container environment

OpenDAL provides a pre-configured [dev container](https://containers.dev/) that could be used in [GitHub Codespaces](https://github.com/features/codespaces), [VSCode](https://code.visualstudio.com/), [JetBrains](https://www.jetbrains.com/remote-development/gateway/), [JupyterLab](https://jupyterlab.readthedocs.io/en/stable/). Please pick up your favourite runtime environment.

The fastest way is:

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/apache/opendal?quickstart=1&machine=standardLinux32gb)

### Bring your own toolbox

To build OpenDAL Zig binding locally, you need:

- [Zig](https://ziglang.org/download) 0.14.0 or higher

> **Note**:
>
> 0.14.0 is not released yet. You can use master instead before the official 0.14.0 released.

## Build

First, build the C bindings:

```shell
zig build libopendal_c
```

> **Note**:
>
> - `zig build` adds the header file `opendal.h` under `../c/include`
> - The library is under `../../target/debug` or `../../target/release` after building.

## Test

To build and run the tests.

```shell
zig build test --summary all
```

```text
Opendal BDD test (0.83ms)
Semantic Analyzer (0.01ms)

2 of 2 tests passed

Slowest 2 tests: 
  0.01ms	Semantic Analyzer
  0.83ms	Opendal BDD test
Error Tests (0.01ms)
Semantic Analyzer (0.00ms)
operator basic operations (0.83ms)
operator advanced operations (0.19ms)
sync operations (0.08ms)

5 of 5 tests passed

Slowest 5 tests: 
  0.00ms	Semantic Analyzer
  0.01ms	Error Tests
  0.08ms	sync operations
  0.19ms	operator advanced operations
  0.83ms	operator basic operations
Build Summary: 9/9 steps succeeded
test success
+- run make success 12s MaxRSS:519M
|  +- libopendal_c_cmake success
|     +- run cmake success 99ms MaxRSS:14M
+- run test success 3ms MaxRSS:15M
|  +- zig test Debug native success 31s MaxRSS:289M
|     +- translate-c success 25ms MaxRSS:53M
+- run bdd_test success 2ms MaxRSS:14M
   +- zig test bdd_test Debug native success 31s MaxRSS:265M
      +- translate-c (reused)
```

