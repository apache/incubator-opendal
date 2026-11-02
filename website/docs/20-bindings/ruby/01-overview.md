---
title: Ruby
sidebar_label: Overview
slug: /bindings/ruby
description: The OpenDAL Ruby binding — install it, see what it supports, and where to go next.
---

# Apache OpenDAL™ Ruby Binding

A native Ruby binding for OpenDAL: access S3, GCS, Azure Blob, the local
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

Released and published to [RubyGems](https://rubygems.org/gems/opendal).

## Capabilities

- **Synchronous** — every operation runs on the calling thread; there is no
  async API.
- **One API for every service** — the gem bundles all backends, selected by a
  scheme string at construction time.
- **File-like `IO`** — `op.open(path, "r")` returns a familiar
  read/write/seek/`readline` object you can stream through.
- **Middlewares** (OpenDAL layers) for retry, concurrency limits, throttling,
  and timeouts — see [Going to production](./05-production.md).

## Installation

```shell
gem install opendal
```

Or add it to your `Gemfile`:

```ruby
# Gemfile
gem "opendal"
```

## Useful Links

- **API reference**: [opendal.apache.org/docs/ruby](https://opendal.apache.org/docs/ruby/)
- **Services & configuration**: [/services](/services)
- **Rust core docs**: [docs.rs/opendal](https://docs.rs/opendal/latest/opendal/index.html)
- **Source & examples**: [`bindings/ruby/`](https://github.com/apache/opendal/tree/main/bindings/ruby)

## Next steps

1. [Getting started](./02-getting-started.md) — a runnable program, then a real backend.
2. [Connecting to your storage](./03-connecting.md) — build an operator for any service.
3. [Common tasks](./04-tasks.md) — read, write, stream, list, copy, and rename.
4. [Going to production](./05-production.md) — middlewares, errors, and capability checks.
