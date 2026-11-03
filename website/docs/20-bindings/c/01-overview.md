---
title: C
sidebar_label: Overview
slug: /bindings/c
description: The OpenDAL C binding — experimental, built from source, blocking API over 50+ storage backends.
---

# Apache OpenDAL™ C Binding

A C binding for OpenDAL, giving C programs blocking access to S3, GCS, Azure Blob,
the local filesystem, and [50+ more services](/services) through one API backed by
the Rust core.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::caution Experimental
This binding is **experimental and a work in progress**. The API surface is limited,
there are no pre-built packages, and breaking changes may occur between releases.
It must be built from source.
:::

## Status

Unreleased and experimental. The C binding exposes a blocking-only API generated
via `cbindgen` from the Rust core. There is no async interface and no package
registry distribution; you must compile the shared library yourself.

## Capabilities

- **Blocking operations** — write, read, stat, delete, copy, rename, list, and
  create-dir through a single `opendal_operator *`.
- **Streaming reader and writer** — `opendal_reader` and `opendal_writer` for
  chunk-at-a-time I/O with seek support on the reader.
- **All services via build flags** — enable individual backends with `-DFEATURES`
  at CMake time.
- **C11 compatible** — the header `opendal.h` requires C11 or later (and a C++14
  compiler for the Rust-generated glue).
- **Explicit memory ownership** — every heap-allocated value has a matching
  `*_free` function; results carry an `opendal_error *` that is `NULL` on success.

## Building

Requirements: a C11/C++14 compiler, CMake, and a Rust toolchain.

```shell
cd bindings/c
mkdir -p build && cd build

# Build with the default memory + fs services
cmake ..
make

# Enable additional services
cmake .. -DFEATURES="opendal/services-s3,opendal/services-gcs"
make
```

The shared library lands in `../../target/debug/` and the header is at
`bindings/c/include/opendal.h`.

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/c](https://opendal.apache.org/docs/bindings/c)
- **Services & configuration**: [/services](/services)
- **Source & examples**: [`bindings/c/`](https://github.com/apache/opendal/tree/main/bindings/c)
- **Header file**: [`bindings/c/include/opendal.h`](https://github.com/apache/opendal/blob/main/bindings/c/include/opendal.h)
- **Contributing**: [`CONTRIBUTING.md`](https://github.com/apache/opendal/blob/main/CONTRIBUTING.md)

## Next steps

1. [Getting started](./02-getting-started.md) — build an operator, write and read
   data, handle errors, and free resources.
