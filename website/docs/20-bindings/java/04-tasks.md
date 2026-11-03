---
title: Common tasks
sidebar_label: Common tasks
description: Task-oriented recipes for OpenDAL in Java — read, write, stream, list, delete, copy, and presign.
---

# Common tasks

Recipes for the things you actually do with storage. Each works on every
service. They assume an `Operator op` built as in
[Getting started](./02-getting-started.md). For full signatures, see the
[API reference](https://opendal.apache.org/docs/java/).

A few conventions used throughout:

- Paths are relative to the operator's root; a trailing `/` means a directory
  (`logs/app/` is a directory, `logs/app` is a file).
- `read` returns `byte[]`; `write` takes a `String` or `byte[]`.
- The synchronous `Operator` is shown here. `AsyncOperator` mirrors these verbs,
  returning a `CompletableFuture` from each call.

## Read a whole file

```java
byte[] data = op.read("path/to/file");
String text = new String(data);
```

## Read part of a file

`read(path, offset, length)` reads a range:

```java
byte[] data = op.read("path/to/file", 0, 1024);
```

For repeated reads from the same file, create an `OperatorReader`. It uses a
Rust core reader and lets each call select an independent byte range:

```java
import org.apache.opendal.OperatorReader;
import org.apache.opendal.ReaderOptions;

ReaderOptions options = ReaderOptions.builder()
        .chunk(8 * 1024 * 1024L)
        .concurrent(4)
        .prefetch(2)
        .build();
try (OperatorReader reader = op.createReader("path/to/file", options)) {
    byte[] first = reader.read(0, 1024);
    byte[] next = reader.read(1024, 1024);
}
```

A positive `chunk` enables internal range requests; `concurrent` limits those
requests and `prefetch` counts completed chunks buffered ahead. Setting
`concurrent` alone keeps unchunked streaming. A reader does not snapshot the
file, so subsequent reads may observe changes.

`ReaderOptions` omits `gap` because Java does not expose multi-range reads.

`ReaderOptions` also supports `version`, `ifMatch`, `ifNoneMatch`,
`ifVersionMatch`, `ifVersionNotMatch`, `ifModifiedSince`, and
`ifUnmodifiedSince`. The time conditions accept `java.time.Instant`.
Version selection and conditions require service support. Unsupported options
fail with `Unsupported`; a failed condition on an existing file fails with
`ConditionNotMatch`. Conditions apply to all requests through the reader,
including its streams. Errors may surface when creating the reader
or while reading from it.

## Stream a large file

Don't load gigabytes into memory — read through an `InputStream` in chunks:

```java
import java.io.InputStream;

try (InputStream in = op.createInputStream("big.bin")) {
    byte[] buffer = new byte[8192];
    int n;
    while ((n = in.read(buffer)) != -1) {
        // process buffer[0..n]
    }
}
```

For a configured reader, use `reader.createInputStream()` for the whole file
or `reader.createInputStream(offset, length)` for a range. A length of `-1`
reads to the end; zero selects an empty range. Each stream has its own cursor.
Close each reader and stream separately: closing a reader does not close its
streams, and closing the operator does not close its readers.

## Write a whole file

```java
op.write("path/to/file", "Hello, World!");
op.write("path/to/file", new byte[] {0, 1, 2, 3});
```

## Stream a large upload

Write incrementally through an `OutputStream`; closing it commits the upload:

```java
import java.io.OutputStream;

try (OutputStream out = op.createOutputStream("big.bin")) {
    out.write(firstChunk);
    out.write(secondChunk);
}
```

## Check metadata

```java
import org.apache.opendal.Metadata;

Metadata meta = op.stat("path/to/file");
System.out.println(meta.getContentLength() + " bytes, dir = " + meta.isDir());
```

## List a directory

`list` returns the entries under a path:

```java
import org.apache.opendal.Entry;

for (Entry entry : op.list("dir/")) {
    System.out.println(entry.getPath() + " " + entry.getMetadata().getMode());
}
```

## Walk a tree recursively

Pass a `ListOptions` with `recursive(true)`:

```java
import org.apache.opendal.Entry;
import org.apache.opendal.ListOptions;

ListOptions options = ListOptions.builder().recursive(true).build();
for (Entry entry : op.list("dir/", options)) {
    System.out.println(entry.getPath());
}
```

## Delete a file or a whole tree

```java
op.delete("path/to/file");   // single path; succeeds even if missing
op.removeAll("dir/");        // a path and everything under it
```

## Create a directory

```java
op.createDir("path/to/dir/");   // the trailing slash is required
```

## Copy and rename

```java
op.copy("from.txt", "to.txt");
op.rename("old.txt", "new.txt");
```

Both require a service that supports them; see
[capability checks](./05-production.md#capability-checks).

## Generate a presigned URL

Hand a time-limited URL to a third party so they can access an object without
your credentials. Presigning is part of the **async** API:

```java
import java.time.Duration;
import org.apache.opendal.AsyncOperator;
import org.apache.opendal.PresignedRequest;

try (AsyncOperator op = AsyncOperator.of("s3", conf)) {
    PresignedRequest req =
        op.presignRead("path/to/file", Duration.ofHours(1)).join();
    System.out.println(req.getMethod() + " " + req.getUri());
    System.out.println(req.getHeaders());
}
```

`presignWrite` and `presignStat` produce presigned requests for the matching
operations.

Pass `ReadOptions` to set response headers, for example to give a download a
filename:

```java
import org.apache.opendal.ReadOptions;

try (AsyncOperator op = AsyncOperator.of("s3", conf)) {
    ReadOptions options = ReadOptions.builder()
        .overrideContentDisposition("attachment; filename=\"report.csv\"")
        .overrideContentType("text/csv")
        .overrideCacheControl("private, max-age=60")
        .build();
    PresignedRequest req =
        op.presignRead("reports/latest.csv", Duration.ofHours(1), options).join();
}
```

`presignStat` accepts `StatOptions`, and `presignWrite` accepts `WriteOptions`.
The overloads without options retain their existing behavior. Support for each
option depends on the storage service. Send the returned method and headers
along with the URI when using the presigned request.
</content>
