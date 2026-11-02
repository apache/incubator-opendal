---
title: Getting started
sidebar_label: Getting started
description: Run your first OpenDAL program in Rust, then point it at a real storage backend.
---

# Getting started

## Your first program

This program builds an operator, writes a file, reads it back, inspects its
metadata, and deletes it. It runs against the in-memory service with no
credentials, so you can paste it into a fresh project and run it as-is.

```toml
# Cargo.toml
[dependencies]
opendal = "0.57"
tokio = { version = "1", features = ["full"] }
```

```rust file=core/examples/getting-started/src/main.rs region=quickstart
```

## Point it at a real backend

Only the service changes; the operations stay identical. To use S3, enable its
feature and configure the builder:

```toml
opendal = { version = "0.57", features = ["services-s3"] }
```

```rust
use opendal::services;
use opendal::Operator;

let op = Operator::new(
    services::S3::default()
        .bucket("my-bucket")
        .region("us-east-1"),
)?;

op.write("hello.txt", "Hello from S3!").await?;
```

Each service is gated behind its own `services-*` feature, so enable the ones
you use. The next page, [Connecting to your storage](./03-connecting.md), covers
the construction options and credentials in depth; [Services](/services) lists
every backend and its configuration keys.

## Blocking vs async {#blocking-vs-async}

OpenDAL is async-first. [`Operator`] exposes `async` methods and needs an async
runtime (Tokio by default).

For synchronous code, enable the `blocking` feature and use
[`blocking::Operator`]. Build it from an async operator, then call the same verbs
without `.await`:

```toml
opendal = { version = "0.57", features = ["services-memory", "blocking"] }
```

```rust
use opendal::blocking;
use opendal::services;
use opendal::Operator;

// Build the async operator first, outside any async runtime.
let op = Operator::new(services::Memory::default())?;
let bop = blocking::Operator::new(op)?;

bop.write("hello.txt", "Hello, World!")?;
let bytes = bop.read("hello.txt")?;
```

The blocking operator drives the async one on a managed runtime, so construct it
outside of an existing async context.

[`Operator`]: https://docs.rs/opendal/latest/opendal/struct.Operator.html
[`blocking::Operator`]: https://docs.rs/opendal/latest/opendal/blocking/struct.Operator.html
