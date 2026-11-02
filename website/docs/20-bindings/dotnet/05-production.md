---
title: Going to production
sidebar_label: Going to production
description: Make OpenDAL robust in .NET — layers, error handling, capability checks, and executor lifetime.
---

# Going to production

The basics read and write data. Production code also has to survive transient
failures, bound its resource use, handle errors precisely, and manage native
resource lifetime.

## Layers

A layer wraps an operator to add cross-cutting behavior without touching your
storage code. `WithLayer(...)` returns a new operator with the layer applied;
chain calls to compose several:

```csharp
using OpenDAL;
using OpenDAL.Layer;

using var baseOp = new Operator("memory");

using var op = baseOp
    .WithLayer(new ConcurrentLimitLayer(64))
    .WithLayer(new RetryLayer
    {
        MaxTimes = 5,
        MinDelay = TimeSpan.FromMilliseconds(100),
        MaxDelay = TimeSpan.FromSeconds(5),
        Factor = 2f,
        Jitter = true,
    })
    .WithLayer(new TimeoutLayer
    {
        Timeout = TimeSpan.FromSeconds(30),
        IoTimeout = TimeSpan.FromSeconds(5),
    });
```

The .NET binding exposes these layers in `OpenDAL.Layer`:

| Layer | What it does |
|-------|--------------|
| `RetryLayer` | Retries transient failures with exponential backoff (`MaxTimes`, `MinDelay`, `MaxDelay`, `Factor`, `Jitter`). |
| `TimeoutLayer` | Bounds slow calls with a total `Timeout` and a per-I/O `IoTimeout`. |
| `ConcurrentLimitLayer` | Caps the number of concurrent operations (constructor takes the permit count). |
| `ThrottleLayer` | Rate-limits read and write byte flow (constructor takes bytes per second and the burst size). |
| `MimeGuessLayer` | Fills in `Content-Type` from the file extension when the caller did not set one. |
| `CapabilityOverrideLayer` | Overrides reported capabilities. |

See [Concepts](../../03-concepts.mdx#layer) for the model.

## Error handling

Most failures surface as `OpenDALException` with a typed `Code` of type
`ErrorCode`. Match on the code instead of inspecting messages:

```csharp
using OpenDAL;

try
{
    await op.ReadAsync("maybe-missing.txt");
}
catch (OpenDALException ex) when (ex.Code == ErrorCode.NotFound)
{
    // handle the missing object
}
```

Common codes include `NotFound`, `PermissionDenied`, `AlreadyExists`,
`ConditionNotMatch`, `RateLimited`, and `Unsupported`. Unknown native codes are
normalized to `ErrorCode.Unexpected`.

## Capability checks {#capability-checks}

Not every service supports every operation. Query what a backend can do through
`op.Info.Capability` before calling optional operations like `Copy`, `Rename`,
or presign:

```csharp
var cap = op.Info.Capability;
if (cap.PresignRead)
{
    var req = await op.PresignReadAsync("a.txt", TimeSpan.FromMinutes(5));
}
```

Calling an unsupported operation throws `OpenDALException` with
`ErrorCode.Unsupported`, so capability checks are an optimization, not a
requirement for safety. `Info` also exposes the operator's `Scheme`, `Root`, and
`Name`.

## Executor and lifetime {#executor-and-lifetime}

The binding wraps native handles, so deterministic disposal matters:

- Prefer `using` for `Operator`, `Executor`, and stream instances.
- An operator binds its executor at construction and keeps the runtime alive on
  the native side. Disposing an `Executor` never affects operators already
  bound to it — it only makes constructing new operators with it throw
  `ObjectDisposedException`.
- Dispose an `Operator` only after its awaited operations complete; disposal
  while work is in flight is not supported.
- If you do not bind an `Executor`, OpenDAL uses a shared default executor.
- Layered operators (`WithLayer`) and `Duplicate()` handles inherit the base
  operator's executor.

See [Getting started — Executors](./02-getting-started.md#executors) for how to
create and bind one.

## Path conventions

- Use backend-native object keys, for example `a/b/c.txt`.
- For directory-like operations (`CreateDir`, directory `Stat`, listing a
  directory root) prefer trailing-slash paths such as `logs/`.
