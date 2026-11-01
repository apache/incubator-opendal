---
title: Common tasks
sidebar_label: Common tasks
description: Task-oriented recipes for OpenDAL in Node.js — read, write, stream, list, delete, copy, rename, and presign.
---

# Common tasks

Recipes for the things you actually do with storage. Each works on every
service. They assume an `op` built as in
[Getting started](./02-getting-started.md). For full signatures, see the
[API reference](https://opendal.apache.org/docs/nodejs/).

A few conventions used throughout:

- Paths are relative to the operator's root; a trailing `/` means a directory
  (`logs/app/` is a directory, `logs/app` is a file).
- `read` returns a `Buffer`; `write` takes a `string` or a `Buffer`.
- Every verb has a `*Sync` variant (`readSync`, `writeSync`, …) for blocking code.

## Read a whole file

```javascript
const bs = await op.read("path/to/file");   // Buffer
const text = bs.toString();
```

## Read part of a file

Pass an `offset` and `size` (both `bigint`):

```javascript
const bs = await op.read("path/to/file", { offset: 0n, size: 1024n });
```

## Stream a large file

Don't buffer gigabytes in memory — open a reader and consume it as a Node
`Readable` stream:

```javascript
import { pipeline } from "node:stream/promises";
import { createWriteStream } from "node:fs";

const reader = await op.reader("big.bin");
const rs = reader.createReadStream();
await pipeline(rs, createWriteStream("/tmp/big.bin"));
```

## Write a whole file

```javascript
await op.write("path/to/file", "Hello, World!");
await op.write("path/to/file", Buffer.from([0, 1, 2, 3]));
```

Pass write options as a third argument, for example a content type:

```javascript
await op.write("path/to/file", data, { contentType: "application/json" });
```

## Stream a large upload

Open a writer and drive it with a Node `Writable` stream. Closing the stream
commits the upload:

```javascript
import { pipeline } from "node:stream/promises";
import { createReadStream } from "node:fs";

const writer = await op.writer("big.bin");
const ws = writer.createWriteStream();
await pipeline(createReadStream("/tmp/big.bin"), ws);
```

## Upload concurrently

For large objects on services with multipart support, upload parts in parallel:

```javascript
await op.write("big.bin", data, { concurrent: 8 });
```

## Check existence and metadata

```javascript
if (await op.exists("path/to/file")) {
  const meta = await op.stat("path/to/file");
  console.log(meta.contentLength, meta.isDirectory());
}
```

`Metadata` exposes getters like `contentLength`, `contentType`, `etag`, and
`lastModified`, plus `isFile()` and `isDirectory()`.

## List a directory

`list` returns an array of entries under a path. The path must end with `/`:

```javascript
const entries = await op.list("dir/");
for (const entry of entries) {
  console.log(entry.path(), entry.metadata().isFile());
}
```

For large directories, stream entries with a lister instead of collecting them
into an array:

```javascript
const lister = await op.lister("dir/");
let entry;
while ((entry = await lister.next()) !== null) {
  console.log(entry.path());
}
```

## Walk a tree recursively

```javascript
const entries = await op.list("dir/", { recursive: true });
```

`op.lister("dir/", { recursive: true })` gives the streaming equivalent.

## Delete a file or a whole tree

```javascript
await op.delete("path/to/file");      // single path; succeeds even if missing
await op.remove(["a.txt", "b.txt"]);  // a batch of paths
await op.removeAll("dir/");           // a path and everything under it
```

`delete` is idempotent — deleting a missing path succeeds rather than failing.

## Create a directory

```javascript
await op.createDir("path/to/dir/");   // the trailing slash is required
```

## Copy and rename

```javascript
await op.copy("from.txt", "to.txt");
await op.rename("old.txt", "new.txt");
```

Both require a service that supports them; see
[capability checks](./05-production.md#capability-checks).

## Generate a presigned URL

Hand a time-limited URL to a third party so they can access an object without
your credentials. The `expires` argument is in seconds:

```javascript
const req = await op.presignRead("path/to/file", 3600);  // expires in 3600s
console.log(req.method, req.url);
console.log(req.headers);
```

`presignWrite`, `presignStat`, and `presignDelete` are available too, on
services that support presigning.
</content>
