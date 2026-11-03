---
title: Haskell
sidebar_label: Overview
slug: /bindings/haskell
description: The OpenDAL Haskell binding — experimental status, capabilities, and where to start.
---

# Apache OpenDAL™ Haskell Binding

A Haskell binding for OpenDAL: access S3, GCS, Azure Blob, the local filesystem,
and [50+ more services](/services) through one API, with the Rust core underneath.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::note
This binding has its own version number, independent of the Rust core. When
checking for updates or compatibility, always refer to this binding's version
rather than the core version.
:::

## Status

**Experimental / WIP.** The binding is unreleased and not yet published to
Hackage. The API may change without notice. Use it to experiment or contribute,
but do not rely on it in production.

## Capabilities

- **Blocking IO** — all operations run in `IO`; there is no async API yet.
- **`OperatorT` monad** — compose operations with `do`-notation; errors surface
  as `Left OpenDALError` through `ExceptT` under the hood.
- **Raw `IO` functions** — every operation is also available as a plain `IO`
  function returning `Either OpenDALError a` for direct use without the monad
  transformer.
- **Core operations** — read, write, stat, delete, copy, rename, list (shallow
  and recursive scan), create directory, remove-all, writer (append and
  overwrite).
- **Logging** — optional structured logging via `co-log`.
- **Services** — whichever services the bundled Rust library is compiled with;
  the memory service works out of the box for testing.

## Build

The binding is not on Hackage; build from source inside `bindings/haskell/`:

```shell
# Build the Rust FFI layer and the Haskell library
cabal build

# Run the test suite
cabal test

# Generate Haddock documentation
cabal haddock
```

Requires a working Rust toolchain (`cargo`) in addition to GHC and `cabal`.
The build has been tested with GHC 9.4.8.

## Useful Links

- **Haddock reference**: generated locally with `cabal haddock`
- **Services & configuration**: [/services](/services)
- **Source**: [`bindings/haskell/`](https://github.com/apache/opendal/tree/main/bindings/haskell)
- **Contributing**: [`CONTRIBUTING.md`](https://github.com/apache/opendal/blob/main/bindings/haskell/CONTRIBUTING.md)

## Next steps

1. [Getting started](./02-getting-started.md) — build an operator, read and write, handle errors.
