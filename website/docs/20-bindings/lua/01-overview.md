---
title: Lua
sidebar_label: Overview
slug: /bindings/lua
description: The OpenDAL Lua binding — experimental status, what it supports, how to build it, and where to go next.
---

# Apache OpenDAL™ Lua Binding

A Lua binding for OpenDAL: access the local filesystem, in-memory storage, S3,
and [50+ more services](/services) through one API, backed by the Rust core.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::warning Experimental / WIP
This binding is **unreleased and experimental**. APIs may change without notice.
It is not yet published to LuaRocks. Use it for evaluation and contribution, not
for production workloads.
:::

## Status

Experimental / work in progress. Not yet published to LuaRocks. The binding
version is independent of the Rust core version.

## Capabilities

- **Blocking API** — all operations are synchronous from Lua's perspective.
- **String-based I/O** — `write` takes a Lua string; `read` returns a Lua string.
- **Core operations** — `read`, `write`, `delete`, `stat`, `rename`,
  `create_dir`, `is_exist`.
- **Metadata** — `content_length`, `is_file`, `is_dir` from `stat` results.
- **Any service** that OpenDAL's Rust core supports, including `fs`, `memory`,
  S3, GCS, and the full list at [/services](/services).

Not yet available: async API, listing, streaming, presigned URLs, layers.

## Build and install

LuaRocks publishing is planned but not yet done. Build from source:

```bash
# Step 1: build the native library (default Lua version is 5.2).
cd bindings/lua
cargo build --package opendal-lua --release

# Step 2: copy it to your Lua shared-library directory.
# Adjust the path for your Lua version and OS.
cp ../../target/release/libopendal_lua.so /usr/lib/lua/5.2/opendal.so
```

Alternatively, use LuaRocks with the bundled rockspec:

```bash
cd bindings/lua
luarocks make
```

**Lua version**: the binding currently supports Lua 5.2 only.
The default feature is `lua52`. Other Lua versions (`lua51`, `lua53`, `lua54`,
`luajit`, `luau`) are not yet wired up as Cargo features in this binding.

## Useful Links

- **User guide**: [opendal.apache.org/docs/bindings/lua](https://opendal.apache.org/docs/bindings/lua)
- **Services & configuration**: [/services](/services)
- **Source & examples**: [`bindings/lua/`](https://github.com/apache/opendal/tree/main/bindings/lua)

## Next steps

1. [Getting started](./02-getting-started.md) — create an operator, write, read,
   and inspect metadata.
