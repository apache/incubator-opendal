---
title: Dart
sidebar_label: Overview
slug: /bindings/dart
description: The OpenDAL Dart binding — experimental, stdlib-style API for accessing storage services from Dart/Flutter.
---

# Apache OpenDAL™ Dart Binding

A Dart binding for OpenDAL: access S3, GCS, Azure Blob, the local filesystem,
and [50+ more services](/services) through one API, backed by the Rust core via
[flutter_rust_bridge](https://github.com/fzyzcjy/flutter_rust_bridge).

The API follows the `dart:io` stdlib style — `Storage`, `File`, and `Directory`
objects that should feel familiar to any Dart developer.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::caution Experimental
This binding is a work in progress. The public API may change between releases.
It is not yet published to pub.dev. Expect rough edges and missing features
compared to the Rust core or the Python binding.
:::

## Capabilities

- **Async and sync** — every `File` and `Directory` method has both an `async`
  (`Future<T>`) and a blocking (`*Sync`) variant.
- **File operations** — `read`, `write`, `exists`, `stat`, `delete`, `rename`.
- **Directory operations** — `create`, `exists`, `stat`, `delete`, `rename`.
- **stdlib-style API** — `Storage.initFile()` and `Storage.initDir()` return
  factory functions that mirror `dart:io`.

The binding currently exposes read, write, stat, exists, delete, rename, and
createDir. Listing, copy, and presigned URLs are not yet surfaced.

## Installation

This binding is not yet published to pub.dev. Add it as a path dependency from
the repository, or build it from source following the
[Development guide](https://github.com/apache/opendal/tree/main/bindings/dart#development).

The native Rust library must be compiled separately:

```shell
cd bindings/dart/rust
cargo build -r
```

## Useful Links

- **Source & examples**: [`bindings/dart/`](https://github.com/apache/opendal/tree/main/bindings/dart)
- **Services & configuration**: [/services](/services)
- **Concepts**: [Concepts](../../03-concepts.mdx)

## Next steps

1. [Getting started](./02-getting-started.md) — build and run a minimal program.
