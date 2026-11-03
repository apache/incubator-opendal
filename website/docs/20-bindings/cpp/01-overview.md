---
title: C++
sidebar_label: Overview
slug: /bindings/cpp
description: The OpenDAL C++ binding — experimental/WIP, built from source via CMake.
---

# Apache OpenDAL™ C++ Binding

A C++ binding for OpenDAL: access S3, GCS, Azure Blob, HDFS, the local
filesystem, and [50+ more services](/services) through one API, backed by the
Rust core.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::warning Experimental
This binding is **experimental and a work in progress**. There is no published
package; you must build from source with CMake. The API may change without
notice between releases. It is not yet recommended for production use.
:::

## Status

Unreleased. Built from source only. Only **Clang** and **AppleClang** are
currently supported as compilers; MSVC and GCC are not yet tested.

## Capabilities

- **Sync API** (`opendal.hpp`) — `opendal::Operator` with read, write, stat,
  list, copy, rename, remove, and directory creation.
- **Async API** (`opendal_async.hpp`, opt-in via `-DOPENDAL_ENABLE_ASYNC=ON`,
  requires C++20) — `opendal::async::Operator` exposing the same operations
  as futures.
- **Streaming** — `Reader` / `ReaderStream` for large-object reads without an
  intermediate copy; `Lister` / `Lister::Iterator` for directory traversal.
- **Capability introspection** — `Operator::Info()` returns a `Capability`
  struct so you can check at runtime which features a backend supports.
- **Services** controlled at build time via the `OPENDAL_FEATURES` CMake
  variable (e.g. `"opendal/services-s3,opendal/services-memory"`).

## Integration via CMake

### FetchContent (recommended)

```cmake
include(FetchContent)

FetchContent_Declare(
  opendal-cpp
  GIT_REPOSITORY https://github.com/apache/opendal.git
  GIT_TAG        v0.40.0
  SOURCE_SUBDIR  bindings/cpp
)
FetchContent_MakeAvailable(opendal-cpp)

target_link_libraries(your_target opendal_cpp)
```

### Vendored source

```shell
mkdir third_party && cd third_party
git clone https://github.com/apache/opendal.git
cd opendal && git checkout v0.40.0
```

```cmake
add_subdirectory(third_party/opendal/bindings/cpp)
target_link_libraries(your_target opendal_cpp)
```

### Prerequisites

- CMake ≥ 3.22
- Clang or AppleClang with C++17 support (C++20 required for async)

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/cpp](https://opendal.apache.org/docs/bindings/cpp)
- **Services & configuration**: [/services](/services)
- **Source**: [`bindings/cpp/`](https://github.com/apache/opendal/tree/main/bindings/cpp)
- **Examples**: [`examples/cpp/`](https://github.com/apache/opendal/tree/main/examples/cpp)

## Next steps

1. [Getting started](./02-getting-started.md) — a runnable program, then a real backend.
