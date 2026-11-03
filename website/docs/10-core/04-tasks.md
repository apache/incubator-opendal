---
title: Common tasks
sidebar_label: Common tasks
description: Task-oriented recipes for OpenDAL in Rust — read, write, conditional operations, stream, list, delete, copy, and presign.
---

# Common tasks

Recipes for the things you actually do with storage. They assume an
`op: Operator` built as in [Getting started](./02-getting-started.md). Support
for optional operations and advanced options varies by service; check the
matching [capability](./05-production.md#capability-checks) before using one.
For full method signatures and options, follow the links to the
[API reference][operator].

A few conventions used throughout:

- Paths are relative to the operator's root; a trailing `/` means a directory
  (`logs/app/` is a directory, `logs/app` is a file).
- Reads return a [`Buffer`]; writes accept anything that converts into one
  (`&str`, `String`, `Vec<u8>`, `Bytes`).
- Most verbs have a `*_with` companion (`read_with`, `write_with`, …) for extra
  options like ranges, conditions, content type, and concurrency.

## Read a whole file

```rust
let bytes = op.read("path/to/file").await?;
let text = String::from_utf8(bytes.to_vec())?;
```

## Read part of a file

```rust
let bytes = op.read_with("path/to/file").range(0..1024).await?;
```

## Read conditionally

Use an ETag to make the read succeed only while the object still has the ETag
you inspected:

```rust
let meta = op.stat("config.json").await?;
if let Some(etag) = meta.etag() {
    let bytes = op.read_with("config.json").if_match(etag).await?;
}
```

If the ETag no longer matches between `stat` and `read_with`, the read returns
`ErrorKind::ConditionNotMatch`. `read_with` and `reader_with` also support
`if_none_match`, `if_modified_since`, and `if_unmodified_since`. The same
conditions are available on `stat_with` for conditional metadata checks. See
[Conditional operations] for the portable condition and error contract.

## Stream a large file

Don't buffer gigabytes in memory — stream chunks instead:

```rust
use futures::TryStreamExt;

let mut stream = op.reader("big.bin").await?.into_bytes_stream(..).await?;
while let Some(chunk) = stream.try_next().await? {
    // process chunk (bytes::Bytes)
}
```

## Write a whole file

```rust
let _meta = op.write("path/to/file", "Hello, World!").await?;
let _meta = op.write("path/to/file", vec![0u8; 1024]).await?;
```

## Write conditionally

Use `if_match` for optimistic concurrency control. The write succeeds only if
the target still has the ETag returned by `stat`:

```rust
let meta = op.stat("config.json").await?;
if let Some(etag) = meta.etag() {
    let _meta = op
        .write_with("config.json", r#"{"enabled":true}"#)
        .if_match(etag)
        .await?;
}
```

Use `if_not_exists` to create an object without overwriting an existing one:

```rust
let _meta = op
    .write_with("jobs/123.json", r#"{"state":"queued"}"#)
    .if_not_exists(true)
    .await?;
```

A false condition returns `ErrorKind::ConditionNotMatch`. `if_none_match` is
also available on services that advertise `write_with_if_none_match`. See
[Conditional operations] for atomicity semantics and the behavior when the
file does not exist.

## Stream a large upload

Use a [`Writer`] for data produced incrementally. Call `write` repeatedly, then
`close` to commit (use `abort` to discard):

```rust
let mut writer = op.writer("big.bin").await?;
writer.write(first_chunk).await?;
writer.write(second_chunk).await?;
let _meta = writer.close().await?;
```

## Upload concurrently

For large objects on services with multipart support, upload parts in parallel:

```rust
let mut writer = op.writer_with("big.bin").concurrent(8).await?;
writer.write(data).await?;
writer.close().await?;
```

## Check existence and metadata

```rust
if op.exists("path/to/file").await? {
    let meta = op.stat("path/to/file").await?;
    println!("{} bytes, dir = {}", meta.content_length(), meta.is_dir());
}
```

## List a directory

`list` returns the direct children of a directory:

```rust
for entry in op.list("dir/").await? {
    println!("{} ({:?})", entry.path(), entry.metadata().mode());
}
```

For large directories, stream entries with a [`Lister`] instead of collecting
them into a `Vec`:

```rust
use futures::TryStreamExt;

let mut lister = op.lister("dir/").await?;
while let Some(entry) = lister.try_next().await? {
    println!("{}", entry.path());
}
```

## Walk a tree recursively

```rust
let entries = op.list_with("dir/").recursive(true).await?;
```

`lister_with("dir/").recursive(true)` gives the streaming equivalent.

## Delete a file or a whole tree

```rust
op.delete("path/to/file").await?;   // single path; idempotent
op.remove_all("dir/").await?;       // a path and everything under it
```

`delete` succeeds even if the path does not exist.

## Delete a specific version

Version-aware services can delete one stored version without deleting other
versions of the same path:

```rust
let meta = op.stat("report.csv").await?;
if let Some(version) = meta.version() {
    op.delete_with("report.csv").version(version).await?;
}
```

Check `delete_with_version` before using this option. A version-scoped delete
remains idempotent if that version is missing.

## Create a directory

```rust
op.create_dir("path/to/dir/").await?;   // the trailing slash is required
```

## Copy and rename

```rust
let _meta = op.copy("from.txt", "to.txt").await?;
op.rename("old.txt", "new.txt").await?;
```

Both operate within a single operator and require a service that supports them;
see [capability checks](./05-production.md#capability-checks).

## Copy or rename without overwriting

Some services can make copy or rename fail instead of replacing an existing
target:

```rust
let _meta = op
    .copy_with("from.txt", "to.txt")
    .if_not_exists(true)
    .await?;

op.rename_with("old.txt", "new.txt")
    .if_not_exists(true)
    .await?;
```

Check `copy_with_if_not_exists` or `rename_with_if_not_exists` before using
these conditions. If the target exists, the operation returns
`ErrorKind::ConditionNotMatch`. Conditions guard the destination; see
[Conditional operations] for the complete contract.

## Generate a presigned URL

Hand a time-limited URL to a third party so they can access an object without
your credentials:

```rust
use std::time::Duration;

let req = op.presign_read("path/to/file", Duration::from_secs(3600)).await?;
// req.method(), req.uri(), req.header() describe the HTTP request to make
```

[operator]: https://docs.rs/opendal/latest/opendal/struct.Operator.html
[`Buffer`]: https://docs.rs/opendal/latest/opendal/struct.Buffer.html
[`Writer`]: https://docs.rs/opendal/latest/opendal/struct.Writer.html
[`Lister`]: https://docs.rs/opendal/latest/opendal/struct.Lister.html
[Conditional operations]: /docs/specifications/conditional-operations/
