---
title: OCaml
sidebar_label: Overview
slug: /bindings/ocaml
description: The OpenDAL OCaml binding — experimental WIP, build it from source, and see what it exposes.
---

# Apache OpenDAL™ OCaml Binding

An OCaml binding for OpenDAL: access S3, GCS, Azure Blob, the local
filesystem, and [50+ more services](/services) through one API, backed by the
Rust core.

New to the model behind the API? Read [Concepts](../../03-concepts.mdx) first —
the shared storage semantics and path and operation concepts every binding
expresses through its own native API.

:::warning Experimental / Work in Progress
This binding is **unreleased and experimental**. APIs may change without notice.
It is not published to opam. Build from source only.
:::

## Status

Unreleased — work in progress. OCaml 4.10 or later (OCaml 5 not yet supported)
due to an upstream constraint in `ocaml-rs`.

## Capabilities

- **Blocking API** — all operations are synchronous; no async runtime required.
- **Full result-type error handling** — every fallible function returns
  `('a, string) result`; no exceptions for storage errors.
- **Streaming readers and writers** — `Operator.reader` / `Operator.writer` for
  data that does not fit in memory.
- **Directory listing** — batch (`Operator.list`) and streaming
  (`Operator.lister` + `Operator.Lister.next`) variants.
- **Capability introspection** — check what a backend supports at runtime via
  `Operator.info` and the `Operator.Capability` module.
- **Services**: every backend the Rust core supports is available; the binding
  is built against the full Rust crate.

## Building from source

The binding has no opam release yet. Install the toolchain, then build with dune:

```bash
# 1. Install opam (if not already installed)
bash -c "sh <(curl -fsSL https://raw.githubusercontent.com/ocaml/opam/master/shell/install.sh)"

# 2. Create a 4.x switch (OCaml 5 is not supported)
opam switch create opendal-ocaml4.14 ocaml-base-compiler.4.14.0
eval $(opam env)

# 3. Install development tools (optional but recommended)
opam install -y utop odoc ounit2 ocaml-lsp-server ocamlformat ocamlformat-rpc

# 4. Build
cd bindings/ocaml
dune build

# 5. Run tests
dune test
```

You also need a working Rust toolchain (stable) for the `build.rs` step that
compiles the Rust core into a native library.

## Useful Links

- **Source**: [`bindings/ocaml/`](https://github.com/apache/opendal/tree/main/bindings/ocaml)
- **Services & configuration**: [/services](/services)
- **OCaml module reference**: `lib/operator.mli` in the source tree

## Next steps

1. [Getting started](./02-getting-started.md) — create an operator, read, write,
   and handle errors.
