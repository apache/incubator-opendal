---
title: Connecting to your storage
sidebar_label: Connecting to your storage
description: Build an OpenDAL operator in .NET for any backend — scheme + dictionary, typed config, and credentials.
---

# Connecting to your storage

Switching backends is a configuration change, not a code change. For the
configuration keys of a specific service, see [Services](/services).

The binding bundles every service — there are no build flags. Once the binding
is referenced, every backend is available.

## Build an operator

### From a scheme and dictionary

The first argument is the service scheme; configuration follows as a
`Dictionary<string, string>` whose keys match the service's configuration fields:

```csharp
using OpenDAL;

using var op = new Operator("s3", new Dictionary<string, string>
{
    ["bucket"] = "my-bucket",
    ["region"] = "us-east-1",
    ["endpoint"] = "https://s3.amazonaws.com",
});
```

A local filesystem operator roots every path under one directory:

```csharp
using var fs = new Operator("fs", new Dictionary<string, string>
{
    ["root"] = "/tmp/opendal",
});
```

### From a typed service config

Each service also has a strongly typed config class in `OpenDAL.ServiceConfig`.
It is converted to the same key/value options internally, so the result is
identical — pick this form for compile-time-checked, discoverable properties:

```csharp
using OpenDAL;
using OpenDAL.ServiceConfig;

using var fs = new Operator(new FsServiceConfig
{
    Root = "/tmp/opendal",
});
```

Available scheme names and their keys are defined by OpenDAL; see the
[Scheme reference](https://docs.rs/opendal/latest/opendal/enum.Scheme.html) and
[Services](/services).

### With a dedicated executor

Both constructors accept an optional `Executor` that binds the operator to a
dedicated Tokio runtime, for example to isolate a heavy workload. Leaving it
out keeps the shared default:

```csharp
using var executor = new Executor(4);
using var op = new Operator("s3", options, executor);
```

See [Getting started — Executors](./02-getting-started.md#executors).

## Credentials

Credentials are just more configuration keys. Set them in the dictionary (or on
the typed config) — for example S3's `access_key_id` and `secret_access_key`:

```csharp
using var op = new Operator("s3", new Dictionary<string, string>
{
    ["bucket"] = "my-bucket",
    ["region"] = "us-east-1",
    ["access_key_id"] = Environment.GetEnvironmentVariable("AWS_ACCESS_KEY_ID")!,
    ["secret_access_key"] = Environment.GetEnvironmentVariable("AWS_SECRET_ACCESS_KEY")!,
});
```

Some services also load credentials from their platform's default sources (for
S3, the standard AWS environment variables and profiles). See each service's
page under [Services](/services) for the exact keys and credential behavior.

Avoid hard-coding secrets in source. Read them from the environment or a secret
manager and pass them in when you build the operator.

## Verify the connection

`Check` lists the service root and throws `OpenDALException` when the service
is unreachable or rejects the request. Call it at startup so wrong credentials
or endpoints fail there instead of on the first real operation:

```csharp
await op.CheckAsync();
```

## One operator per service and root

An operator maps to one service, one root path, and one executor. To work with
two buckets, two roots, or two runtimes, build two operators — they are
lightweight handles. Use `Duplicate()` to create another handle to the same
backend configuration. Always dispose operators with `using` or an explicit
`Dispose()`.
