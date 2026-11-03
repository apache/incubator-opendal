---
title: Common tasks
sidebar_label: Common tasks
description: Task-oriented recipes for OpenDAL in .NET — read, write, stream, list, delete, copy, and presign.
---

# Common tasks

Recipes for the things you actually do with storage. Each works on every
service. They assume an `op` built as in
[Getting started](./02-getting-started.md).

A few conventions used throughout:

- Paths are relative to the operator's root; a trailing `/` means a directory
  (`logs/app/` is a directory, `logs/app` is a file).
- `Read` returns `byte[]`; `Write` takes `byte[]`.
- Every method has an `…Async` counterpart that returns a `Task` and accepts a
  `CancellationToken`. The recipes below show the sync form; add `Async`/`await`
  for the async API.
- Option types live in `OpenDAL.Options`.

## Read a whole file

```csharp
using System.Text;

byte[] bytes = op.Read("path/to/file");
string text = Encoding.UTF8.GetString(bytes);
```

When you only consume the payload, `Read<T>` skips the array: your callback
gets a `ReadOnlySequence<byte>` over native memory and parses in place. The
memory is only valid inside the callback:

```csharp
using System.Text;

string text = op.Read("notes.txt", sequence => Encoding.UTF8.GetString(sequence));
```

Pick `byte[]` when you need to keep the bytes, and the callback form when you
only parse them — it works with `Utf8JsonReader` and any other
`ReadOnlySequence`-aware parser. `ReadAsync<T>` is the async counterpart.

## Read part of a file

Pass a `ReadOptions` with an offset and length:

```csharp
using OpenDAL.Options;

byte[] head = op.Read("path/to/file", new ReadOptions { Offset = 0, Length = 1024 });
```

## Write a whole file

```csharp
using System.Text;

op.Write("path/to/file", Encoding.UTF8.GetBytes("Hello, World!"));
```

`Write` returns the metadata of the new object, so its size or ETag is
available without a follow-up `Stat`.

When you are producing the payload rather than holding it, the fill callback
serializes straight into native memory with no intermediate `byte[]` — the
callback receives a native-backed `IBufferWriter<byte>`:

```csharp
using System.Text.Json;

op.Write("data/config.json", writer =>
{
    using var json = new Utf8JsonWriter(writer);
    JsonSerializer.Serialize(json, config);
});
```

Pick `byte[]` when you already hold the bytes, and the fill callback when a
serializer produces them. An optional `sizeHint` pre-sizes the buffer, and
`WriteAsync` accepts the same callback.

`WriteOptions` carry content headers, conditions, append mode, and concurrency:

```csharp
using OpenDAL.Options;

await op.WriteAsync("docs/readme.txt", content, new WriteOptions
{
    ContentType = "text/plain",
    CacheControl = "no-cache",
    IfNotExists = true,
});
```

## Stream a large file

Don't buffer gigabytes in memory — open a `Stream` and read in chunks.
`OpenReadStream` returns an `OperatorInputStream`:

```csharp
using var reader = op.OpenReadStream("big.bin");
using var ms = new MemoryStream();
reader.CopyTo(ms);
```

## Stream a large upload

`OpenWriteStream` returns an `OperatorOutputStream`. Write incrementally, then
call `CompleteAsync` (or `Complete`) to flush, close the native writer, and
surface any write error — an error raised during plain disposal is lost:

```csharp
using System.Text;

await using (var writer = op.OpenWriteStream("big.bin"))
{
    var chunk = Encoding.UTF8.GetBytes("stream-data");
    await writer.WriteAsync(chunk, 0, chunk.Length);
    await writer.CompleteAsync();
}
```

`OpenWriteStream` also accepts a `bufferSize` argument that controls how much is
buffered before flushing to the backend.

## Check existence and metadata

`Exists` returns `false` instead of throwing when the path is missing:

```csharp
if (op.Exists("path/to/file"))
{
    Console.WriteLine("found");
}
```

`Stat` returns the metadata:

```csharp
var meta = op.Stat("path/to/file");
Console.WriteLine($"{meta.ContentLength} bytes, dir = {meta.IsDir}");
```

`UserMetadata` holds the pairs written through `WriteOptions.UserMetadata`
on services that support it. `IsDeleted` and `IsCurrent` describe entries
returned by listings that include deleted objects or old versions.

A missing path throws `OpenDALException` with `ErrorCode.NotFound`; see
[error handling](./05-production.md#error-handling).

## List a directory

`List` returns the entries under a path:

```csharp
foreach (var entry in op.List("dir/"))
{
    Console.WriteLine($"{entry.Path} ({entry.Metadata.Mode})");
}
```

## Walk a tree recursively

```csharp
using OpenDAL.Options;

var entries = op.List("dir/", new ListOptions { Recursive = true });
```

## Delete a file or a whole tree

```csharp
op.Delete("path/to/file");   // single path
op.RemoveAll("dir/");        // a path and everything under it
```

## Create a directory

```csharp
op.CreateDir("path/to/dir/");   // the trailing slash is required
```

## Copy and rename

```csharp
op.Copy("from.txt", "to.txt");
op.Rename("old.txt", "new.txt");
```

`Copy` returns the metadata of the new object.
Both operate within a single operator and require a service that supports them;
see [capability checks](./05-production.md#capability-checks).

## Generate a presigned URL

Hand a time-limited URL to a third party so they can access an object without
your credentials. Presigning is part of the **async** API:

```csharp
var request = await op.PresignReadAsync("data/file.txt", TimeSpan.FromMinutes(10));

Console.WriteLine(request.Method);
Console.WriteLine(request.Uri);
foreach (var header in request.Headers)
{
    Console.WriteLine($"{header.Key}: {header.Value}");
}
```

The available presign methods are `PresignReadAsync`, `PresignWriteAsync`,
`PresignStatAsync`, and `PresignDeleteAsync`.
