---
title: Getting started
sidebar_label: Getting started
description: Write, read, and list with the OpenDAL D binding against the in-memory backend.
---

# Getting started

## Prerequisites

Build the D binding from source before running any code. From `bindings/d/`:

```shell
OPENDAL_TEST=memory dub build -b release
```

See [Overview](./01-overview.md) for the full build requirements.

## Your first program

The example below creates an `Operator` against the in-memory backend (no
credentials needed), writes a file, reads it back, checks that it exists,
inspects its metadata, and deletes it.

```d file=bindings/d/examples/getting-started/source/app.d region=quickstart
```

## Listing a directory

`createDir` and `list` work together:

```d
import opendal;
import std.stdio : writeln;

void main() @safe
{
    auto options = new OperatorOptions();
    auto op = Operator("memory", options);

    op.createDir("logs/");
    op.write("logs/a.txt", cast(ubyte[]) "first".dup);
    op.write("logs/b.txt", cast(ubyte[]) "second".dup);

    Entry[] entries = op.list("logs/");
    foreach (e; entries)
        writeln(e.path(), "  name=", e.name());
}
```

## Pointing at a real backend

Only the `OperatorOptions` change; all method calls stay identical.

```d
import opendal;

void main() @safe
{
    auto options = new OperatorOptions();
    options.set("root", "/tmp/mydata");

    auto op = Operator("fs", options);
    op.write("hello.txt", cast(ubyte[]) "Hello from the filesystem!".dup);
}
```

`OPENDAL_TEST` is read by the build script at **build time** to compile the
named service into the C library. To use the `fs` backend you must build with
it enabled:

```shell
OPENDAL_TEST=fs OPENDAL_FS_ROOT=/tmp/mydata dub build -b release
```

At runtime the backend is selected by the scheme string passed to `Operator`
(e.g. `"fs"`), and backend-specific settings are passed through
`OperatorOptions` (e.g. `options.set("root", "/tmp/mydata")`). See
[Services](/services) for every available backend and its configuration keys.

## Parallel operations

Pass `useParallel = true` to spawn a `TaskPool` and use `writeParallel`,
`readParallel`, and `listParallel`:

```d
import opendal;

void main() @safe
{
    auto options = new OperatorOptions();
    auto op = Operator("memory", options, true /* useParallel */);

    op.writeParallel("hello.txt", cast(ubyte[]) "data".dup);
    ubyte[] result = op.readParallel("hello.txt");
}
```
