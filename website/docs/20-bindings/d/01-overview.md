---
title: D
sidebar_label: Overview
slug: /bindings/d
description: The OpenDAL D binding — early-stage and experimental, built from source on top of the C binding.
---

# Apache OpenDAL™ D Binding

A D language binding for OpenDAL, wrapping the [C binding](../c) to give D programs
access to the same unified storage API as every other OpenDAL binding.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::warning
This binding is **early-stage and experimental**. It is not yet released or
published to a package registry. The API surface is minimal, the build requires
building from source, and breaking changes can happen at any time.
:::

## Status

Unreleased. Must be built from source. Suitable for experimentation and
contribution, not for production use.

## Capabilities

The current API exposes a single `Operator` struct with the following methods:

| Method | Description |
|---|---|
| `write(path, data)` | Write `ubyte[]` to `path` |
| `read(path)` | Return `ubyte[]` from `path` |
| `remove(path)` | Delete the object at `path` |
| `exists(path)` | Return `bool` indicating whether `path` exists |
| `stat(path)` | Return a `Metadata` struct for `path` |
| `list(path)` | Return an `Entry[]` listing the directory at `path` |
| `createDir(path)` | Create a directory at `path` |
| `rename(src, dest)` | Rename / move `src` to `dest` |
| `copy(src, dest)` | Copy `src` to `dest` |

Three operations have parallel variants: `writeParallel`, `readParallel`,
and `listParallel`. Each dispatches the call to a `std.parallelism.TaskPool`.
Parallel variants are enabled by passing `useParallel = true` to the
`Operator` constructor.

`Metadata` exposes `contentLength()`, `isFile()`, `isDir()`, and
`lastModified()`. `Entry` exposes `path()` and `name()`.

Most methods are `@trusted` wrappers around the underlying C binding.
`writeParallel` is `@safe`. `Entry.path()` and `Entry.name()` carry no
safety attribute (default `@system`) because they call into the C binding
without a `@trusted` annotation. There is no async API at this stage.

## Building from Source

You need:

- A D compiler: [dmd, ldc, or gdc](https://dlang.org/download)
- Rust and Cargo (to build the underlying C library)

```shell
# From the bindings/d/ directory

# Build libopendal_c and the D library (release mode)
OPENDAL_TEST=memory dub build -b release

# Build and run unit tests against the memory backend
OPENDAL_TEST=memory dub test
```

The pre-build script (`build.d`) reads `OPENDAL_TEST` at **build time** to
decide which Cargo `--features` (i.e. which storage backends) get compiled
into the C library. The resulting static library is copied into `lib/` before
`dub` links the D code. Omitting `OPENDAL_TEST` builds only core
functionality with no storage backends.

At runtime the test suite also reads `OPENDAL_TEST` to select which backend
scheme to pass to `Operator(...)`. The runtime backend is constructed with
`Operator(scheme, options)` where `options` holds service-specific keys such
as `root`.

To test against a service other than `memory`, set `OPENDAL_TEST` and the
matching `OPENDAL_{SERVICE}_*` configuration variables:

```shell
OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp/test dub test
```

## Useful Links

- **Source**: [`bindings/d/`](https://github.com/apache/opendal/tree/main/bindings/d)
- **Services & configuration**: [/services](/services)

## Next Steps

- [Getting started](./02-getting-started.md) — write, read, and list with the memory backend.
