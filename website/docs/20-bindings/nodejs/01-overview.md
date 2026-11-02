---
title: Node.js
sidebar_label: Overview
slug: /bindings/nodejs
description: The OpenDAL Node.js binding — install it, see what it supports, and where to go next.
---

# Apache OpenDAL™ Node.js Binding

A native Node.js binding for OpenDAL: access S3, GCS, Azure Blob, HDFS, the local
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

Released and stable, published to [npm](https://www.npmjs.com/package/opendal).

## Capabilities

- **Promise-based async API** — every operation returns a `Promise`; `await` it.
- **Synchronous variants** — each verb has a `*Sync` form (`readSync`,
  `writeSync`, …) for blocking code.
- **Streaming** reads and writes via Node `Readable`/`Writable` streams for data
  that does not fit in memory.
- **All services included** — the package bundles every backend; there are no
  build flags to enable.
- **Composable layers** for retry, timeout, logging, throttle, and concurrency
  limits — see [Going to production](./05-production.md).
- **Presigned URLs** for services that support them (S3, GCS, Azblob, …).

## Installation

```shell
npm install opendal
```

## Useful Links

- **API reference**: [opendal.apache.org/docs/nodejs](https://opendal.apache.org/docs/nodejs/)
- **Services & configuration**: [/services](/services)
- **Upgrade guide**: [`upgrade.md`](https://github.com/apache/opendal/blob/main/bindings/nodejs/upgrade.md)
- **Source & examples**: [`bindings/nodejs/`](https://github.com/apache/opendal/tree/main/bindings/nodejs) · [`bindings/nodejs/examples/`](https://github.com/apache/opendal/tree/main/bindings/nodejs/examples)

## Next steps

1. [Getting started](./02-getting-started.md) — a runnable program, then a real backend.
2. [Connecting to your storage](./03-connecting.md) — build an operator for any service.
3. [Common tasks](./04-tasks.md) — read, write, stream, list, copy, and presign.
4. [Going to production](./05-production.md) — retry, timeouts, errors, observability.
</content>
</invoke>
