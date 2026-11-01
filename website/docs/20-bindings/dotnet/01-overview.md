---
title: .NET
sidebar_label: Overview
slug: /bindings/dotnet
description: The OpenDAL .NET binding — install it, see what it supports, and where to go next.
---

# Apache OpenDAL™ .NET Binding

A native .NET binding for OpenDAL: access S3, GCS, Azure Blob, the local
filesystem, and [50+ more services](/services) through one API, with the
performance of the Rust core underneath.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::note
This binding has its own version number, independent of the Rust core. Check
compatibility against the binding's version, not the core's.
:::

## Status

Unreleased. The package is not yet published to NuGet, so today you build the
binding from source (see [Installation](#installation)). The intended package id
is `Apache.OpenDAL`. The API is functional and covered by tests.

## Capabilities

- **Sync and async** — every operation has a blocking method and an
  `…Async` (`Task`-based) counterpart with `CancellationToken` support.
- **`Stream` wrappers** — `OpenReadStream` / `OpenWriteStream` return ordinary
  .NET `Stream`s you can pipe through.
- **All services included** — the binding bundles every backend; there are no
  build flags to enable individual services.
- **Composable layers** for retry, timeout, and concurrency limits.
- **Presigned URLs** for services that support them (async API).

## Installation

The binding targets `net8.0` and `net10.0`. Until a NuGet package is published,
build it from source — you need the [.NET SDK](https://dotnet.microsoft.com/en-us/download/dotnet)
and a Rust toolchain for the native library:

```bash
cargo build
dotnet build
```

See [Getting started](./02-getting-started.md) for a runnable example and the
[build details](https://github.com/apache/opendal/tree/main/bindings/dotnet).

## Useful Links

- **Services & configuration**: [/services](/services)
- **OpenDAL Scheme reference**: [docs.rs/opendal — enum.Scheme](https://docs.rs/opendal/latest/opendal/enum.Scheme.html)
- **Source & examples**: [`bindings/dotnet/`](https://github.com/apache/opendal/tree/main/bindings/dotnet)

## Next steps

1. [Getting started](./02-getting-started.md) — a runnable program, then a real backend.
2. [Connecting to your storage](./03-connecting.md) — build an operator for any service.
3. [Common tasks](./04-tasks.md) — read, write, stream, list, presign, and more.
4. [Going to production](./05-production.md) — layers, errors, and capability checks.
