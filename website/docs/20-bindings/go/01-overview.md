---
title: Go
sidebar_label: Overview
slug: /bindings/go
description: The OpenDAL Go binding — install it, see what it supports, and where to go next.
---

# Apache OpenDAL™ Go Binding

A native Go binding for OpenDAL: access S3, GCS, Azure Blob, the local
filesystem, and [many more services](/services) through one API, with the
performance of the Rust core underneath. It is built on `opendal-c` via
[purego](https://github.com/ebitengine/purego) and libffi, so **no CGO** is
required.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::note
This binding has its own version number, independent of the Rust core. Check
compatibility against the binding's version, not the core's.
:::

## Status

Released and usable, published to
[pkg.go.dev](https://pkg.go.dev/github.com/apache/opendal/bindings/go).

## Capabilities

- **Synchronous API** — every operation is a blocking method that returns a Go
  `error`; there is no async variant.
- **Streaming** reads and writes through `io.ReadSeekCloser` and `io.WriteCloser`.
- **Per-scheme packages** — each service is a small companion module under
  `github.com/apache/opendal-go-services/<scheme>` that you import alongside the
  binding.
- **Presigned URLs** for services that support them, returned as
  `*net/http.Request`.

## Installation

The binding requires **libffi** to be installed on your system.

```shell
go get github.com/apache/opendal/bindings/go@latest
```

Each service lives in its own companion module. Add the schemes you use, for
example the in-memory service:

```shell
go get github.com/apache/opendal-go-services/memory
```

## Useful Links

- **API reference**: [pkg.go.dev](https://pkg.go.dev/github.com/apache/opendal/bindings/go)
- **Services & configuration**: [/services](/services)
- **Source & examples**: [`bindings/go/`](https://github.com/apache/opendal/tree/main/bindings/go)
- **Scheme packages**: [`opendal-go-services`](https://github.com/apache/opendal-go-services)

## Next steps

1. [Getting started](./02-getting-started.md) — a runnable program, then a real backend.
2. [Connecting to your storage](./03-connecting.md) — build an operator for any service.
3. [Common tasks](./04-tasks.md) — read, write, stream, list, copy, rename, and presign.
4. [Going to production](./05-production.md) — retries, timeouts, errors, and capability checks.
