---
title: Zig
sidebar_label: Overview
slug: /bindings/zig
description: The OpenDAL Zig binding — early-stage, built from source, wraps the C API to expose blocking storage operations.
---

# Apache OpenDAL™ Zig Binding

A Zig binding for OpenDAL, giving Zig programs blocking access to S3, GCS, Azure Blob,
the local filesystem, and [50+ more services](/services) through one API backed by the
Rust core.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::caution Early-stage / Experimental
This binding is **early-stage and a work in progress**. The API is incomplete,
there are no pre-built packages, and breaking changes may occur. It must be built
from source. It is not yet published to any package registry.
:::

## Status

Unreleased and experimental (version `0.0.1`). The Zig binding wraps the OpenDAL C
binding (`libopendal_c`) using Zig's `translate-c` mechanism, then exposes a thin
idiomatic Zig layer on top. Only synchronous (blocking) operations are implemented;
an async interface using coroutines is noted as WIP in the source.

Requires Zig 0.14.0 or higher.

## Capabilities

Operations available through `Operator`:

- **write** — write bytes to a path
- **read** — read bytes from a path
- **stat** — fetch metadata (size, type, content-type, etag, last-modified)
- **delete** — remove a path
- **exists** — check whether a path exists
- **list** — iterate directory entries (returns a `Lister` yielding `Entry` values)
- **createDir** — create a directory path
- **rename** — rename/move a path (supported only on backends that expose this capability)
- **copy** — copy a path (supported only on backends that expose this capability)

All operations return Zig errors (`OpendalError`) rather than raw C error codes.

`info()` / `OperatorInfo` exist in the source but are currently broken (a compile error in `src/opendal.zig` references an undefined variable) and cannot be used until the bug is fixed.

The binding also exposes the raw C API under the `opendal.c` namespace for cases where
the thin layer does not yet cover what you need.

## Building

Requirements: Zig 0.14.0 or higher, a C/C++ compiler, CMake, and a Rust toolchain.

```shell
# From bindings/zig/

# 1. Build the underlying C library (runs cmake + make under bindings/c)
zig build libopendal_c

# 2. Build and run the unit tests
zig build test --summary all
```

The C library lands in `bindings/c/target/debug/` (or `release/` with
`-Doptimize=ReleaseSafe`). The header is at `bindings/c/include/opendal.h`.

To build in release mode:

```shell
zig build libopendal_c -Doptimize=ReleaseSafe
zig build test -Doptimize=ReleaseSafe --summary all
```

## Using the module in your project

Add the module to your `build.zig`:

```zig
const opendal_dep = b.dependency("opendal", .{
    .target = target,
    .optimize = optimize,
});
exe.root_module.addImport("opendal", opendal_dep.module("opendal"));
```

Then import it:

```zig
const opendal = @import("opendal");
```

## Useful Links

- **Source**: [`bindings/zig/`](https://github.com/apache/opendal/tree/main/bindings/zig)
- **Services & configuration**: [/services](/services)
- **C binding (underlying layer)**: [`bindings/c/`](https://github.com/apache/opendal/tree/main/bindings/c)
- **Contributing**: [`bindings/zig/CONTRIBUTING.md`](https://github.com/apache/opendal/blob/main/bindings/zig/CONTRIBUTING.md)

## Next steps

1. [Getting started](./02-getting-started.md) — build an operator, write and read data, inspect metadata, and list a directory.
