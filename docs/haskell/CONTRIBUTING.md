# Contributing
- [Contributing](#contributing)
  - [Setup](#setup)
    - [Using a dev container environment](#using-a-dev-container-environment)
    - [Bring your own toolbox](#bring-your-own-toolbox)
  - [Build](#build)
  - [Test](#test)
  - [Doc](#doc)

## Setup

### Using a dev container environment
OpenDAL provides a pre-configured [dev container](https://containers.dev/) that could be used in [GitHub Codespaces](https://github.com/features/codespaces), [VSCode](https://code.visualstudio.com/), [JetBrains](https://www.jetbrains.com/remote-development/gateway/), [JupyterLab](https://jupyterlab.readthedocs.io/en/stable/). Please pick up your favourite runtime environment.

The fastest way is:

[![Open in GitHub Codespaces](https://github.com/codespaces/badge.svg)](https://codespaces.new/apache/opendal?quickstart=1&machine=standardLinux32gb)

### Bring your own toolbox

The `haskell` binding requires `haskell` and `cabal` to be built. We recommend using the latest stable version for development.

If you are new to `haskell`, we recommend using [GHCup](https://www.haskell.org/ghcup/) to install `haskell` and `cabal`.

For Unix-like systems:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://get-ghcup.haskell.org | sh
```

For Windows:

```powershell
Set-ExecutionPolicy Bypass -Scope Process -Force;[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; try { Invoke-Command -ScriptBlock ([ScriptBlock]::Create((Invoke-WebRequest https://www.haskell.org/ghcup/sh/bootstrap-haskell.ps1 -UseBasicParsing))) -ArgumentList $true } catch { Write-Error $_ }
```

To verify that everything is working properly, run `ghc -V` and `cabal -V`:

```shell
> ghc -V
The Glorious Glasgow Haskell Compilation System, version 9.2.8
> cabal -V
cabal-install version 3.6.2.0
compiled using version 3.6.2.0 of the Cabal library
```

## Build

```shell
cabal build
```

To clean up the build:

```shell
cargo clean
cabal clean
```

## Test

We use [`tasty`](https://hackage.haskell.org/package/tasty) as the test framework. To run the tests:

```shell
cabal test
```

```text
...(Build Info)
Test suite opendal-test: RUNNING...
Test suite opendal-test: PASS
Test suite logged to: 
...(Log Path)
1 of 1 test suites (1 of 1 test cases) passed.
```

## Doc

To generate the documentation:

```shell
cabal haddock
```

If your `cabal` version is greater than `3.8`, you can use `cabal haddock --open` to open the documentation in your browser. Otherwise, you can visit the documentation from `dist-newstyle/build/$ARCH/ghc-$VERSION/opendal-$VERSION/doc/html/opendal/index.html`.
