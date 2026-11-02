---
title: Getting started
sidebar_label: Getting started
description: Run your first OpenDAL program in .NET, then point it at a real storage backend.
---

# Getting started

## Your first program

This program builds an operator, writes a file, reads it back, inspects its
metadata, and deletes it. It runs against the in-memory service with no
credentials, so you can run it as soon as the binding is built.

```csharp file=bindings/dotnet/examples/GettingStarted/Program.cs region=quickstart
```

`Write` takes a `byte[]`, and `Read` returns a `byte[]`. The `using` declaration
disposes the operator and frees its native handle when it goes out of scope —
always dispose `Operator` instances.

## Point it at a real backend

Only the service changes; the operations stay identical. Pass the scheme and a
dictionary of configuration keys:

```csharp
using OpenDAL;
using System.Text;

using var op = new Operator("s3", new Dictionary<string, string>
{
    ["bucket"] = "my-bucket",
    ["region"] = "us-east-1",
});

op.Write("hello.txt", Encoding.UTF8.GetBytes("Hello from S3!"));
Console.WriteLine(Encoding.UTF8.GetString(op.Read("hello.txt")));
```

The binding bundles every service, so there is nothing extra to install. The
next page, [Connecting to your storage](./03-connecting.md), covers construction
and credentials in depth; [Services](/services) lists every backend and its keys.

## Async usage {#blocking-vs-async}

Every operation has an `…Async` counterpart that returns a `Task` and accepts a
`CancellationToken`. Await each call:

```csharp
using OpenDAL;
using System.Text;

using var op = new Operator("memory");

await op.WriteAsync("hello.txt", Encoding.UTF8.GetBytes("Hello, World!"));
var bytes = await op.ReadAsync("hello.txt");
Console.WriteLine(Encoding.UTF8.GetString(bytes));
```

## Executors {#executors}

Every operation runs on a native Tokio runtime. By default OpenDAL uses a shared
executor, so you can call any method without configuring anything. To control
the worker-thread count, create an [`Executor`] and bind it when you construct
the operator:

```csharp
using OpenDAL;
using System.Text;

using var executor = new Executor(2); // two Tokio worker threads
using var op = new Operator("memory", executor: executor);

await op.WriteAsync("hello.txt", Encoding.UTF8.GetBytes("Hello, World!"));
var bytes = await op.ReadAsync("hello.txt");
```

The operator keeps its runtime alive on the native side, so disposing the
`Executor` handle never affects operators already bound to it — it only makes
constructing new operators with it throw `ObjectDisposedException`. One
operator binds one executor; to spread work across two runtimes, construct two
operators. See [Going to production](./05-production.md#executor-and-lifetime)
for the lifetime rules.

[`Executor`]: https://github.com/apache/opendal/blob/main/bindings/dotnet/OpenDAL/Executor.cs
