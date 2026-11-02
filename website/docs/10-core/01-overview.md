---
title: Rust Core
sidebar_label: Overview
slug: /core
description: The Rust core library that powers OpenDAL — install it, see what it supports, and where to go next.
---

# Apache OpenDAL™ Rust Core

`opendal` is the Rust core that every OpenDAL binding is built on. It is the
most complete and most mature surface of the project: every service, every
layer, and the full async API live here first.

New to the model behind the API? Read [Concepts](../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

## Status

Stable and production-ready. The crate follows semver; breaking changes are
batched into minor releases and documented in the [Upgrade Guide].

## Capabilities

- **Async-first**, with an optional [blocking API](./02-getting-started.md#blocking-vs-async).
- **Streaming** reads and writes for data that does not fit in memory.
- **50+ services** behind one API — see [Services](/services) for the full list
  and per-service configuration.
- **Composable layers** for retry, logging, timeout, and metrics — see
  [Going to production](./05-production.md).
- **Presigned URLs** for services that support them (S3, GCS, Azblob, …).

## Installation

```shell
cargo add opendal
```

OpenDAL is modular: each service is its own feature flag. The in-memory service
is always available; enable any other service — including `fs` — with its
`services-*` feature.

```toml
[dependencies]
opendal = { version = "0.57", features = ["services-s3"] }
```

## Useful Links

- **API reference**: [docs.rs/opendal][docs.rs] (release) · [dev build][dev-docs]
- **Specifications**: [current portable contracts][Specifications]
- **Rust core guides**: [concepts, internals, and performance][Rust core guides] (rustdoc)
- **Services & configuration**: [/services](/services)
- **Upgrade guide**: [dev documentation][Upgrade Guide]
- **Changelog**: [dev documentation][Changelog]
- **Source & examples**: [`core/`][source] · [`core/examples/`][examples]

## Next steps

1. [Getting started](./02-getting-started.md) — a runnable program, then a real backend.
2. [Connecting to your storage](./03-connecting.md) — build an operator for any service.
3. [Common tasks](./04-tasks.md) — read, write, stream, list, presign, and more.
4. [Going to production](./05-production.md) — retry, timeouts, errors, observability.

[docs.rs]: https://docs.rs/opendal/
[dev-docs]: https://opendal.apache.org/docs/rust/opendal/
[Specifications]: /docs/specifications/
[Rust core guides]: https://opendal.apache.org/docs/rust/opendal/docs/
[Upgrade Guide]: https://opendal.apache.org/docs/rust/opendal/docs/upgrade/
[Changelog]: https://opendal.apache.org/docs/rust/opendal/docs/changelog/
[source]: https://github.com/apache/opendal/tree/main/core
[examples]: https://github.com/apache/opendal/tree/main/core/examples
